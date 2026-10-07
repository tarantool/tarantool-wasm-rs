from decimal import Decimal
from uuid import UUID
from twr_python_sdk import tarantool as box
class WitWorld:
    def exchange(self):
        row = box.space['js_specials'].get(1)
        assert row[1] == Decimal('-12.34')
        assert row[2] == UUID('f6423bdf-b49e-4913-b361-0740c9702e4b')
        assert row[3] == box.Datetime(-10801, 123456789, 180)
        assert row[4] == box.Interval(years=1, months=200, days=-77)
        assert row[5].stack[0].message == 'JS error'
        assert box.space['js_bigints'].get(4) == (4, 2**64-1, -(2**63))
        box.space['js_specials'].replace((2, *row[1:]))
        return 'ok'
