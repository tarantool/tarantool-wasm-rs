"""Wire vectors from Tarantool and failure cases which need no running host."""

from datetime import datetime, timedelta, timezone
from decimal import Decimal, localcontext
from pathlib import Path
from uuid import UUID
import struct
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "sdk/src"))
from msgpack import ExtType, Timestamp
from msgpack.fallback import Packer
from twr_python_sdk import tarantool as box
from twr_python_sdk.tarantool import _codec


class MessagePackTests(unittest.TestCase):
    def test_decimal_host_vectors_and_context_independence(self):
        vectors = {
            "-12.34": "d6010201234d",
            "0.000000000000000000000000000000000010": "c7030124010c",
            "-0.00": "d501020d",
            "1e40": "c70301d0d81c",
        }
        with localcontext() as context:
            context.prec = 2
            for text, hex_data in vectors.items():
                with self.subTest(text=text):
                    value = Decimal(text)
                    encoded = bytes.fromhex(hex_data)
                    self.assertEqual(box.msgpack.pack(value), encoded)
                    self.assertEqual(
                        box.msgpack.unpack(encoded).as_tuple(), value.as_tuple()
                    )
            value = Decimal("123456789012345678901234567890.12345678")
            self.assertEqual(
                box.msgpack.unpack(box.msgpack.pack(value)).as_tuple(), value.as_tuple()
            )

    def test_decimal_signed_zero_and_alternative_signs(self):
        for sign in (10, 11, 12, 13, 14, 15):
            decoded = box.msgpack.unpack(Packer().pack(ExtType(1, bytes([0, sign]))))
            self.assertEqual(decoded.as_tuple().sign, int(sign in (11, 13)))
        for value in (Decimal("0"), Decimal("-0"), Decimal("0.00"), Decimal("-0.00")):
            self.assertEqual(
                box.msgpack.unpack(box.msgpack.pack(value)).as_tuple(), value.as_tuple()
            )

    def test_decimal_nonfinite_and_scale_errors(self):
        for value in (
            Decimal("NaN"),
            Decimal("sNaN"),
            Decimal("Infinity"),
            Decimal("-Infinity"),
        ):
            with self.assertRaises(ValueError):
                box.msgpack.pack(value)
        for value in (Decimal("1e2147483649"), Decimal("1e-2147483648")):
            with self.assertRaises(ValueError):
                box.msgpack.pack(value)

    def test_uuid_host_vector_and_scalar_key(self):
        value = UUID("f6423bdf-b49e-4913-b361-0740c9702e4b")
        encoded = bytes.fromhex("d802f6423bdfb49e4913b3610740c9702e4b")
        self.assertEqual(box.msgpack.pack(value), encoded)
        self.assertEqual(box.msgpack.unpack(encoded), value)
        self.assertEqual(_codec.unpack(_codec.key(value)), [value])

    def test_datetime_host_vector(self):
        value = box.Datetime(-10801, 123456789, 180)
        encoded = bytes.fromhex("d804cfd5ffffffffffff15cd5b07b4000000")
        self.assertEqual(box.msgpack.pack(value), encoded)
        self.assertEqual(box.msgpack.unpack(encoded), value)
        for value in (
            box.Datetime(0),
            box.Datetime(-1),
            box.Datetime(0, tzindex=947),
            box.Datetime(0, 1999999999),
            box.Datetime(-185604722870400),
            box.Datetime(185480451417600),
        ):
            self.assertEqual(box.msgpack.unpack(box.msgpack.pack(value)), value)

    def test_python_datetime_conversion_is_exact(self):
        for value in (
            datetime(1969, 12, 31, 23, 59, 59, 999999, timezone.utc),
            datetime(2024, 1, 1, 5, 6, 7, 123456, timezone(timedelta(minutes=-210))),
        ):
            converted = box.Datetime.from_datetime(value)
            self.assertEqual(converted.to_datetime(), value)
            self.assertEqual(converted.to_datetime().utcoffset(), value.utcoffset())
            self.assertEqual(box.msgpack.unpack(box.msgpack.pack(value)), converted)
        with self.assertRaises(ValueError):
            box.msgpack.pack(datetime(2024, 1, 1))
        with self.assertRaises(ValueError):
            box.msgpack.pack(
                datetime(2024, 1, 1, tzinfo=timezone(timedelta(seconds=1)))
            )

    def test_datetime_conversion_refuses_precision_loss(self):
        value = box.Datetime(-1, 123456789, 180, 947)
        with self.assertRaises(ValueError):
            value.to_datetime()
        self.assertEqual(value.to_datetime(truncate=True).microsecond, 123456)
        with self.assertRaises(ValueError):
            box.Datetime(0, 1000000000).to_datetime(truncate=True)
        with self.assertRaises(OverflowError):
            box.Datetime(185480451417600).to_datetime()

    def test_datetime_local_year_boundaries(self):
        for value in (
            datetime(1, 1, 1, tzinfo=timezone(timedelta(hours=14))),
            datetime(9999, 12, 31, 23, 59, 59, 999999, timezone(timedelta(hours=-12))),
        ):
            converted = box.Datetime.from_datetime(value)
            self.assertEqual(converted.to_datetime(), value)
            self.assertEqual(converted.to_datetime().utcoffset(), value.utcoffset())
            self.assertEqual(box.msgpack.unpack(box.msgpack.pack(value)), converted)

    def test_datetime_validation(self):
        for kw in (
            {"seconds": True},
            {"seconds": 1.5},
            {"seconds": 185480451417601},
            {"nanoseconds": -1},
            {"nanoseconds": 2000000000},
            {"tzoffset": -721},
            {"tzoffset": 841},
            {"tzindex": 1025},
        ):
            with self.subTest(kw=kw), self.assertRaises((TypeError, ValueError)):
                box.Datetime(**({"seconds": 0} | kw))

    def test_interval_host_vectors(self):
        vectors = [
            (box.Interval(), "c70306010801"),
            (
                box.Interval(years=1, months=200, days=-77),
                "c70b0604000101ccc803d0b30801",
            ),
            (box.Interval(adjust=box.IntervalAdjust.EXCESS), "d40600"),
        ]
        for value, hex_data in vectors:
            with self.subTest(value=value):
                encoded = bytes.fromhex(hex_data)
                self.assertEqual(box.msgpack.pack(value), encoded)
                self.assertEqual(box.msgpack.unpack(encoded), value)
        for mode in box.IntervalAdjust:
            value = box.Interval(
                years=-1,
                months=2,
                weeks=-3,
                days=4,
                hours=-5,
                minutes=6,
                seconds=-7,
                nanoseconds=8,
                adjust=mode,
            )
            self.assertEqual(box.msgpack.unpack(box.msgpack.pack(value)), value)

    def test_interval_ranges_prevent_host_rounding(self):
        for name in ("years", "months", "weeks", "nanoseconds"):
            with self.assertRaises(ValueError):
                box.Interval(**{name: 2**31})
        for name in ("days", "hours", "minutes", "seconds"):
            with self.assertRaises(ValueError):
                box.Interval(**{name: 2**53 + 1})
        for value in (-1, 3, True, 1.5):
            with self.assertRaises((TypeError, ValueError)):
                box.Interval(adjust=value)

    def test_error_stack_metadata_and_future_fields(self):
        value = box.ErrorValue(
            (
                box.ErrorFrame(
                    42,
                    "outer",
                    "CustomError",
                    "outer.lua",
                    11,
                    5,
                    {"uuid": UUID(int=1)},
                    {9: "future frame"},
                ),
                box.ErrorFrame(1, "inner"),
            ),
            {7: "future envelope"},
        )
        self.assertEqual(box.msgpack.unpack(box.msgpack.pack(value)), value)
        raw = box.msgpack.unpack(box.msgpack.pack(value), raw_extensions=True)
        self.assertEqual(raw.code, 3)
        self.assertEqual(box.msgpack.unpack(Packer().pack(raw)), value)

    def test_error_values_validate_metadata(self):
        for args in ((True, "x"), (1, None)):
            with self.assertRaises(TypeError):
                box.ErrorFrame(*args)
        for kwargs in (
            {"fields": {1: "x"}},
            {"extra": {True: "x"}},
            {"extra": {0: "override"}},
            {"line": -1},
        ):
            with self.assertRaises((TypeError, ValueError)):
                box.ErrorFrame(1, "x", **kwargs)
        with self.assertRaises(TypeError):
            box.ErrorValue(())
        with self.assertRaises(ValueError):
            box.ErrorValue((box.ErrorFrame(1, "x"),), {0: []})

    def test_unknown_extensions_and_raw_mode(self):
        value = {
            1: [UUID(int=1), Decimal("1.20"), box.Interval(), ExtType(127, b"opaque")]
        }
        encoded = box.msgpack.pack(value)
        self.assertEqual(box.msgpack.unpack(encoded), value)
        raw = box.msgpack.unpack(encoded, raw_extensions=True)
        self.assertEqual([v.code for v in raw[1]], [2, 1, 6, 127])
        self.assertEqual(box.msgpack.unpack(box.msgpack.pack(raw)), value)
        timestamp = Timestamp(-1, 1)
        self.assertEqual(box.msgpack.unpack(box.msgpack.pack(timestamp)), timestamp)

    def test_malformed_known_extensions_are_rejected(self):
        bad = {
            1: [b"", b"\x00", b"\x00\x1a\x0c", b"\x00\x19", b"\xc0\x1c", b"\xcc"],
            2: [b"", b"opaque", b"x" * 17],
            4: [b"", b"x" * 9, struct.pack("<qihh", 0, -1, 0, 0)],
            6: [
                b"",
                b"\x01",
                b"\x01\x09\x01",
                b"\x01\x00\xc0",
                b"\x00\x00",
                b"\x02\x00\x01\x00\x02",
                b"\x01\xcc\x00\x01",
                b"\x0a",
            ],
            3: [
                b"",
                Packer().pack({}),
                Packer().pack({0: []}),
                Packer().pack({0: [{}]}),
                Packer().pack({0: [{0: "E", 1: "f", 3: 4}]}),
                Packer().pack({False: [{0: "E", 1: "f", 3: "m"}]}),
            ],
        }
        for code, cases in bad.items():
            for data in cases:
                with self.subTest(code=code, data=data):
                    encoded = Packer().pack(ExtType(code, data))
                    with self.assertRaises(ValueError):
                        box.msgpack.unpack(encoded)
                    self.assertEqual(
                        box.msgpack.unpack(encoded, raw_extensions=True),
                        ExtType(code, data),
                    )


if __name__ == "__main__":
    unittest.main()
