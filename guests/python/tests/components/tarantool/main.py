"""Installed-wheel checks against the real Tarantool host."""

import json
import logging
import os
from copy import copy, deepcopy
from twr_python_sdk import tarantool as box
from decimal import Decimal
from uuid import UUID
from datetime import datetime, timedelta, timezone
import _tarantool
from msgpack import ExtType
from wit_world import exports


def raises(expected, function, *args, **kwargs):
    try:
        function(*args, **kwargs)
    except expected as error:
        return error
    raise AssertionError(f"expected {expected.__name__}")


def crud():
    users = box.space["sdk_users"]
    assert users.id > 0 and box.schema_version() > 0
    assert box.space[users.id].id == users.id
    assert users.insert((1, "Alice", 10)) == (1, "Alice", 10)
    assert users.get(1) == (1, "Alice", 10)
    duplicate = raises(box.TarantoolError, users.insert, (1, "Alice", 10))
    assert duplicate.code > 0 and duplicate.message and duplicate.error_type
    assert users.replace((1, "Bob", 20)) == (1, "Bob", 20)
    assert users.update(1, [("+", 2, 10)]) == (1, "Bob", 30)
    assert users.index["name"].get("Bob") == (1, "Bob", 30)
    users.upsert((1, "Ignored", 0), [("+", 2, 5)])
    assert users.get(1) == (1, "Bob", 35)
    users.upsert((2, "Eve", 40), [])
    assert users.delete(2) == (2, "Eve", 40)
    assert users.delete(99) is None and users.update(99, [("=", 1, "x")]) is None
    raises(KeyError, box.space.__getitem__, "sdk_missing")
    raises(KeyError, users.index.__getitem__, "missing")
    raises(TypeError, users.insert, {"id": 2})
    users.truncate()
    assert users.get(1) is None


def indexes():
    users = box.space["sdk_users"]
    for value in ((1, "Alice", 10), (2, "Bob", 20), (3, "Eve", 30)):
        users.replace(value)
    primary = users.index[0]
    assert primary.id == 0 and len(primary) == 3 and primary.memory_size > 0
    assert primary.min() == (1, "Alice", 10)
    assert primary.max() == (3, "Eve", 30)
    assert primary.random(123) in users.select()
    assert primary.count(2) == 1
    assert primary.count(2, mode=box.IteratorType.GREATER_THAN_OR_EQUAL) == 2
    assert users.select(limit=1, offset=1) == [(2, "Bob", 20)]
    assert users.select(limit=0) == [] and users.select(offset=99) == []
    with users.iter() as cursor:
        assert next(cursor)[0] == 1
    assert cursor.closed
    assert list(users.iter()) == users.select()
    assert primary.select(2, mode=box.IteratorType.LESS_THAN) == [(1, "Alice", 10)]
    assert users.index["name"].update("Bob", [("=", 2, 22)]) == (2, "Bob", 22)
    assert users.index["name"].delete("Eve") == (3, "Eve", 30)


def tuples():
    value = (1, "abc", {"x": [None, b"\x00\xff"]})
    with box.tuple(value) as original:
        assert len(original) == 3 and original.byte_size == len(original.to_bytes())
        assert original[0] == 1 and original[-2] == "abc" and original[1:] == value[1:]
        assert original["[2].x[0]"] is None
        assert original["[2].x[1]"] == b"\x00\xff"
        assert tuple(original) == value and original.to_tuple() == value
        raises(IndexError, original.__getitem__, 99)
        raises(KeyError, original.__getitem__, "[99]")
        with copy(original) as retained:
            assert retained.to_tuple() == value
        with original.update([("=", 1, "xyz")]) as updated:
            assert updated.to_tuple() == (1, "xyz", value[2])
        with original.upsert([("+", 0, 100)]) as updated:
            assert updated[0] == 101
        assert original[0] == 1
        with original.format as format:
            original.validate(format)
        with original.iter_fields() as fields:
            raises(TypeError, copy, fields)
            assert fields.position == 0 and next(fields) == 1
            assert fields.seek(1) == "abc" and fields.position == 2
            assert next(fields) == value[2]
            raises(StopIteration, next, fields)
            fields.rewind()
            assert fields.position == 0 and list(fields) == list(value)
            raises(IndexError, fields.seek, 99)
    original.close()
    raises(box.ClosedResourceError, original.to_tuple)


def keys_formats():
    part = box.KeyPart(0, box.FieldType.UNSIGNED, flags=box.KeyPartFlags.IS_NULLABLE)
    with box.key_def([part]) as first, box.key_def(
        [box.KeyPart(1, box.FieldType.STRING)]
    ) as second:
        assert first.parts == (part,) and len(first) == 1
        with copy(first) as copied:
            assert copied.parts == first.parts
        with first.merge(second) as merged:
            assert len(merged) == 2
            merged.validate_key([1])
            merged.validate_key([1, "abc"], full=True)
            raises(box.TarantoolError, merged.validate_key, [1], full=True)
        with box.tuple((1, "abc")) as left, box.tuple((2, "xyz")) as right:
            first.validate_tuple(left)
            assert first.extract_key(left) == (1,)
            assert (
                left.compare(right, first) < 0 and left.compare_with_key(1, first) == 0
            )
            with box.tuple_format([first, second]) as format, copy(format) as retained:
                left.validate(format)
                left.validate(retained)
                with box.tuple(("bad", "abc")) as invalid:
                    raises(box.TarantoolError, invalid.validate, format)
        with box.tuple_format() as default:
            with box.tuple((None,)) as value:
                value.validate(default)
    with box.key_def(
        [box.KeyPart(1, box.FieldType.STRING, collation=box.Collation.UNICODE_CI)]
    ) as collated:
        assert collated.parts[0].collation == box.Collation.UNICODE_CI


def sequences():
    # The test host places the sequence ID in sdk_ids.
    seq = box.sequence(box.space["sdk_ids"].get(1)[1])
    seq.set(20)
    assert seq.current == 20 and seq.next() == 21
    seq.reset()
    assert seq.next() == 1


def diagnostics():
    saved = box.error.new(1, "sdk expected", file="guest.py", line=17)
    assert saved.code == 1 and saved.file == "guest.py" and saved.line == 17
    assert "sdk expected" in box.error.format(saved)
    box.error.set(saved)
    latest = box.error.last()
    assert latest.code == 1 and latest.message == "sdk expected" and latest.line == 17
    box.error.clear()
    assert box.error.last() is None
    assert box.error.new(1, "simple").message == "simple"
    assert box.metrics.uptime() >= 0


def logging_api():
    for level in box.LogLevel:
        box.log.write(
            level, "SDK log " + level.value, context=box.LogContext("sdk_test.py", 1)
        )
    logger = logging.getLogger("sdk-test")
    handler = box.log.handler()
    logger.addHandler(handler)
    logger.setLevel(logging.DEBUG)
    try:
        for level in (
            logging.DEBUG,
            logging.INFO,
            logging.WARNING,
            logging.ERROR,
            logging.CRITICAL,
        ):
            logger.log(level, "SDK logging %s", level)
    finally:
        logger.removeHandler(handler)
        handler.close()


def messagepack():
    value = {
        1: None,
        "bin": b"\x00\xff",
        "values": [False, 2**63, -3, 1.5],
        "ext": ExtType(127, b"opaque"),
    }
    assert box.msgpack.unpack(box.msgpack.pack(value)) == value
    encoded = box.msgpack.from_json('{"a": [1, true]}')
    assert box.msgpack.unpack(encoded) == {"a": [1, True]}
    assert json.loads(box.msgpack.to_json(encoded)) == {"a": [1, True]}
    raises(box.TarantoolError, box.msgpack.from_json, "{")


def extensions_read():
    row = box.space["sdk_specials"].get(1)
    assert row[1] == Decimal("-12.34")
    assert row[2] == UUID("f6423bdf-b49e-4913-b361-0740c9702e4b")
    assert row[3] == box.Datetime(-10801, 123456789, 180)
    assert row[4] == box.Interval(years=1, months=200, days=-77)
    assert isinstance(row[5], box.ErrorValue)
    frame = row[5].stack[0]
    assert frame.code == 1 and frame.message == "extension test"
    assert frame.error_type == "IllegalParams" and frame.line > 0
    assert frame.file.endswith("tarantool_test.lua")
    with box.tuple(row) as value:
        assert value.to_tuple() == row
        assert value[1] == row[1] and value[3] == row[3]
        with value.iter_fields() as fields:
            assert list(fields) == list(row)


def extensions_write():
    for boundary in (
        datetime(1, 1, 1, tzinfo=timezone(timedelta(hours=14))),
        datetime(9999, 12, 31, 23, 59, 59, 999999, timezone(timedelta(hours=-12))),
    ):
        with box.tuple((boundary,)) as value:
            assert value[0].to_datetime() == boundary
    date = datetime(2024, 1, 1, 12, 30, 1, 123456, timezone(timedelta(minutes=180)))
    frame = box.ErrorFrame(
        42, "Python error", "CustomError", "guest.py", 17, fields={"object_name": "sdk"}
    )
    value = (
        2,
        Decimal("12345.6700"),
        UUID(int=1),
        date,
        box.Interval(months=2, days=-1, adjust=box.IntervalAdjust.SNAP),
        box.ErrorValue((frame,)),
    )
    expected = (*value[:3], box.Datetime.from_datetime(date), *value[4:])
    space = box.space["sdk_specials"]
    row = space.replace(value)
    assert row == expected
    assert space.get(2) == expected
    assert space.index["decimal"].get(value[1]) == expected
    assert space.index["uuid"].get(value[2]) == expected
    assert space.index["datetime"].get(date) == expected
    assert space.update(2, [("+", 1, Decimal("0.33"))])[1] == Decimal("12346.0000")
    assert space.index["uuid"].delete(value[2])[0] == 2
    assert space.get(2) is None


def extensions_keys():
    row = box.space["sdk_specials"].get(1)
    for field, kind in (
        (1, box.FieldType.DECIMAL),
        (2, box.FieldType.UUID),
        (3, box.FieldType.DATETIME),
    ):
        with box.key_def([box.KeyPart(field, kind)]) as definition:
            definition.validate_key(row[field], full=True)
            with box.tuple(row) as value:
                assert definition.extract_key(value) == (row[field],)
                assert value.compare_with_key(row[field], definition) == 0
    raw = box.msgpack.unpack(box.msgpack.pack(row), raw_extensions=True)
    assert [value.code for value in raw[1:]] == [1, 2, 4, 6, 3]
    assert box.msgpack.unpack(box.msgpack.pack(raw)) == list(row)


def extensions_callbacks():
    row = box.space["sdk_specials"].get(1)
    with box.lua_callback("sdk_echo") as echo:
        for value in row[1:]:
            assert echo(value) == value
        values = {
            "decimal": row[1],
            "uuid": row[2],
            "datetime": row[3],
            "interval": row[4],
            "error": row[5],
        }
        assert echo(values) == values


def extensions_intervals():
    space = box.space["sdk_intervals"]
    for i, adjust in enumerate(box.IntervalAdjust):
        value = box.Interval(
            years=-1,
            months=2,
            weeks=3,
            days=-4,
            hours=5,
            minutes=-6,
            seconds=7,
            nanoseconds=-8,
            adjust=adjust,
        )
        assert space.replace((i, value)) == (i, value)
        assert space.get(i) == (i, value)
    assert space.replace((9, box.Interval(adjust=box.IntervalAdjust.EXCESS)))[
        1
    ] == box.Interval(adjust=box.IntervalAdjust.EXCESS)


def sessions():
    current = box.session.current()
    assert isinstance(current.id, int) and current.id >= 0
    box.session.broadcast_lua("sdk_event", "{answer = 42}")
    raises(TypeError, current.send, [], {})
    target = box.session[box.space["sdk_ids"].get(2)[1]]
    target.send({0: 0, 1: 77}, {0x30: "sdk"})
    target.send({0: 0, 1: 78})


def callbacks():
    with box.lua_callback("sdk_echo") as callback:
        raises(TypeError, copy, callback)
        raises(TypeError, deepcopy, callback)
        assert callback({"a": [1, 2, True], "text": "hello"}) == {
            "a": [1, 2, True],
            "text": "hello",
        }
        assert callback(None) is None
        assert callback([1, None, 3]) == [1, None, 3]
        assert callback({}) == {}
        assert callback([]) == []
        assert callback({1: None}) == {1: None}
    raises(box.ClosedResourceError, callback, 1)
    with box.lua_callback("sdk_failure") as callback:
        failure = raises(box.LuaCallbackError, callback, 1)
        assert "sdk callback failure" in str(failure)
    with box.lua_callback("sdk_missing") as callback:
        raises(box.LuaCallbackError, callback, 1)
    raises(box.LuaCallbackError, box.lua_callback, "")
    with box.lua_callback("sdk_binary") as callback:
        assert callback(b"\x00\xff") == b"\x00\xff"
    with box.lua_callback("sdk_plain_binary") as callback:
        assert callback(b"\x00\xff") == b"\x00\xff"
    with box.lua_callback("sdk_nulls") as callback:
        assert callback() == [1, None, 3]


def raw_bindings():
    assert _tarantool.database is box._default._bindings.module("database")
    for name in (
        "LogLevel",
        "TransactionIsolation",
        "IteratorType",
        "Collation",
        "FieldType",
    ):
        assert set(getattr(_tarantool.types, name).__members__) == set(
            getattr(box, name).__members__
        )
    encoded = _tarantool.msgpack.from_json("[42]")
    handle = _tarantool.box_tuple.new(encoded)
    try:
        assert box.msgpack.unpack(_tarantool.box_tuple.to_bytes(handle)) == [42]
    finally:
        _tarantool.box_tuple.release(handle)


def transactions():
    users = box.space["sdk_users"]
    assert box.transaction.id is None
    with box.transaction(isolation=box.TransactionIsolation.READ_CONFIRMED):
        assert box.transaction.id is not None
        assert box.transaction.isolation == box.TransactionIsolation.READ_CONFIRMED
        box.transaction.set_isolation(box.TransactionIsolation.BEST_EFFORT)
        raises(RuntimeError, box.transaction().__enter__)
        users.replace((11, "commit", 1))
    assert users.get(11) is not None and box.transaction.id is None
    try:
        with box.transaction():
            users.replace((12, "rollback", 1))
            raise ValueError("rollback")
    except ValueError:
        pass
    assert users.get(12) is None and box.transaction.id is None
    box.transaction.begin()
    users.replace((13, "explicit rollback", 1))
    box.transaction.rollback()
    assert users.get(13) is None
    box.transaction.begin()
    users.replace((14, "explicit commit", 1))
    box.transaction.commit()
    assert users.get(14) is not None
    box.transaction.begin()
    box.transaction.make_synchronous()
    box.transaction.rollback()


CASES = {
    name: value
    for name, value in globals().copy().items()
    if name
    in (
        "crud",
        "indexes",
        "tuples",
        "keys_formats",
        "sequences",
        "diagnostics",
        "logging_api",
        "messagepack",
        "extensions_read",
        "extensions_write",
        "extensions_keys",
        "extensions_callbacks",
        "extensions_intervals",
        "sessions",
        "callbacks",
        "raw_bindings",
        "transactions",
    )
}


class WitWorld:
    def check(self, name):
        CASES[name]()
        return "ok"


class Run(exports.Run):
    def run(self):
        CASES[os.environ.get("SDK_CASE", "transactions")]()
