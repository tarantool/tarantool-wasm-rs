from decimal import Decimal
from uuid import UUID
from twr_python_sdk import tarantool as box

class WitWorld:
    def exchange(self):
        row = box.space['go_specials'].get(1)
        assert row[1] == Decimal('1234567890123456789012345678.9012345678')
        assert row[2] == UUID('01020300-0000-0000-0000-000000000000')
        assert row[3] == box.Datetime(-1, 999999999, 180)
        assert row[4] == box.Interval(years=2, adjust=2)
        assert row[5].stack[0].message == 'go'
        assert box.space['go_bigints'].get(1) == (1, 2**64-1, -(2**63), 2**53)
        box.space['go_specials'].replace((2, *row[1:]))
        return 'ok'

    def roundtrip(self, data):
        return box.msgpack.pack(box.msgpack.unpack(bytes(data)))
