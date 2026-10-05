"""Use the portable MessagePack implementation, including opaque extensions."""

from msgpack.fallback import Packer, unpackb
from msgpack.ext import ExtType, Timestamp


def pack(value):
    return Packer(use_bin_type=True).pack(value)


def unpack(value):
    return unpackb(bytes(value), raw=False, strict_map_key=False)


def array(value):
    if not isinstance(value, (tuple, list)):
        raise TypeError("expected a tuple or list")
    return pack(value)


def key(value):
    if isinstance(value, (ExtType, Timestamp)):
        return array((value,))
    return array(value if isinstance(value, (tuple, list)) else (value,))


def operations(value, *, tuple_api=False):
    if not isinstance(value, (tuple, list)):
        raise TypeError("operations must be a tuple or list")
    result = []
    for operation in value:
        if not isinstance(operation, (tuple, list)) or len(operation) < 2:
            raise ValueError("each operation must contain an operator and field")
        item = list(operation)
        # The runtime's box_tuple_update/upsert C APIs are one-based; the
        # database APIs pass index_base=0. Public Python field numbers are 0-based.
        if tuple_api and isinstance(item[1], int) and item[1] >= 0:
            item[1] += 1
        result.append(item)
    return pack(result)
