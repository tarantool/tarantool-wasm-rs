"""Tarantool MessagePack extensions; payload formats match the host codecs."""

from datetime import datetime
from decimal import Decimal
from struct import pack as struct_pack, unpack as struct_unpack
from uuid import UUID

from msgpack.ext import ExtType
from msgpack.fallback import Packer, Unpacker, unpackb
from msgpack.exceptions import OutOfData

from .values import Datetime, Interval, IntervalAdjust, ErrorFrame, ErrorValue, integer

_INTERVAL_FIELDS = (
    "years",
    "months",
    "weeks",
    "days",
    "hours",
    "minutes",
    "seconds",
    "nanoseconds",
    "adjust",
)


def encode(value):
    if isinstance(value, UUID):
        return ExtType(2, value.bytes)
    if isinstance(value, Decimal):
        if not value.is_finite():
            raise ValueError("Tarantool decimal must be finite")
        sign, digits, exponent = value.as_tuple()
        integer("decimal scale", -exponent, -(2**31), 2**31 - 1)
        # Use the tuple directly: Decimal arithmetic would apply the caller's
        # context and could round the value before serialization.
        nibbles = list(digits) + [13 if sign else 12]
        if len(nibbles) % 2:
            nibbles.insert(0, 0)
        bcd = bytes((a << 4) | b for a, b in zip(nibbles[::2], nibbles[1::2]))
        return ExtType(1, Packer().pack(-exponent) + bcd)
    if isinstance(value, datetime):
        value = Datetime.from_datetime(value)
    if isinstance(value, Datetime):
        data = struct_pack("<q", value.seconds)
        if value.nanoseconds or value.tzoffset or value.tzindex:
            data += struct_pack(
                "<ihh", value.nanoseconds, value.tzoffset, value.tzindex
            )
        return ExtType(4, data)
    if isinstance(value, Interval):
        pairs = [
            (i, getattr(value, name))
            for i, name in enumerate(_INTERVAL_FIELDS)
            if getattr(value, name)
        ]
        packer = Packer()
        data = bytes([len(pairs)]) + b"".join(
            bytes([i]) + packer.pack(int(number)) for i, number in pairs
        )
        return ExtType(6, data)
    if isinstance(value, ErrorValue):
        frames = []
        for frame in value.stack:
            frames.append(
                {
                    **frame.extra,
                    0: frame.error_type,
                    1: frame.file,
                    2: frame.line,
                    3: frame.message,
                    4: frame.errno,
                    5: frame.code,
                    6: frame.fields,
                }
            )
        return ExtType(
            3,
            Packer(use_bin_type=True, default=encode).pack({**value.extra, 0: frames}),
        )
    raise TypeError(f"cannot MessagePack-encode {type(value).__name__}")


def decode(code, data):
    try:
        if code == 2:
            return UUID(bytes=data)
        if code == 1:
            reader = Unpacker()
            reader.feed(data)
            scale = reader.unpack()
            integer("decimal scale", scale, -(2**31), 2**31 - 1)
            bcd = data[reader.tell() :]
            if not bcd or bcd[-1] & 15 not in (10, 11, 12, 13, 14, 15):
                raise ValueError("invalid decimal sign or missing digits")
            digits = [n for byte in bcd for n in (byte >> 4, byte & 15)][:-1]
            if any(n > 9 for n in digits):
                raise ValueError("invalid decimal digit")
            return Decimal((int(bcd[-1] & 15 in (11, 13)), tuple(digits), -scale))
        if code == 4:
            if len(data) == 8:
                return Datetime(struct_unpack("<q", data)[0])
            if len(data) == 16:
                return Datetime(*struct_unpack("<qihh", data))
            raise ValueError("datetime payload must contain 8 or 16 bytes")
        if code == 6:
            if not data or data[0] > 9:
                raise ValueError("invalid interval field count")
            # Field IDs are raw bytes; values are MessagePack integers. IDs are
            # at most 8, so a streaming unpacker can safely read both here.
            reader = Unpacker()
            reader.feed(data[1:])
            values = {"adjust": IntervalAdjust.EXCESS}
            seen = set()
            for _ in range(data[0]):
                key, value = reader.read_bytes(1)[0], reader.unpack()
                integer("interval field ID", key, 0, 8)
                if key in seen:
                    raise ValueError("duplicate interval field")
                seen.add(key)
                integer("interval value", value, -(2**63), 2**63 - 1)
                values[_INTERVAL_FIELDS[key]] = value
            if reader.tell() != len(data) - 1:
                raise ValueError("trailing interval payload")
            return Interval(**values)
        if code == 3:
            value = unpackb(data, raw=False, strict_map_key=False, ext_hook=decode)
            if not isinstance(value, dict) or not isinstance(value.get(0), list):
                raise ValueError("missing error stack")
            for key in value:
                integer("error key", key, 0, 2**64 - 1)
            frames = []
            for item in value[0]:
                if not isinstance(item, dict) or not all(k in item for k in (0, 1, 3)):
                    raise ValueError("missing error frame fields")
                for key in item:
                    integer("error frame key", key, 0, 2**64 - 1)
                frames.append(
                    ErrorFrame(
                        item.get(5, 0),
                        item[3],
                        item[0],
                        item[1],
                        item.get(2, 0),
                        item.get(4, 0),
                        item.get(6, {}),
                        {k: v for k, v in item.items() if k not in range(7)},
                    )
                )
            return ErrorValue(tuple(frames), {k: v for k, v in value.items() if k != 0})
    except (TypeError, ValueError, OutOfData, IndexError) as error:
        raise ValueError(
            f"invalid Tarantool MessagePack extension {code}: {error}"
        ) from error
    return ExtType(code, data)
