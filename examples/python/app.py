from adder import exports
from adder.imports import say, msgpack, session, txn, sequence, key_def, tuple_format
from adder.imports import tuple as tt_tuple
from adder.imports import index as tt_index
from adder.imports import ttbox as box
from adder.imports.docs_adder_types import Space, Index, TxnIsolationLevel, Iterator, IteratorType, Sequence, BoxError, KeyPart, KeyPartFlags
import adder
import json
from time import sleep


def encode(obj):
    try:
        return msgpack.encode(json.dumps(obj).encode('utf-8'))
    except Exception as error:
        say.say_error(f"PY | Error in encode: {json.dumps({'error': str(error)}, indent=2)}")
        raise


def decode(msgpack_bytes):
    try:
        decoded_json_bytes: bytes = msgpack.decode(msgpack_bytes)
        decoded_json_string = decoded_json_bytes.decode('utf-8')
        return json.loads(decoded_json_string)
    except Exception as error:
        say.say_error(f"PY | Error in decode: {json.dumps({'error': str(error)}, indent=2)}")
        raise


def test_iterator():
    test_data = [
        [1, "Alice", 25],
        [2, "Bob", 30],
        [3, "Charlie", 35],
        [4, "David", 40],
        [5, "Eve", 4],
    ]
    space: Space = box.space_by_name("test_space")
    say.say_info(f"PY | Space: {space} {type(space)}")

    box.truncate(space)

    index: Index = box.index_by_name(space, "primary")
    say.say_info(f"PY | Index: {index}")

    res = {}
    for data in test_data:
        sleep(3)
        tuple_id = box.insert(space, encode(data))
        say.say_info(f"PY | Insert: {tuple_id = }")
        res[tuple_id] = data
    
    sleep(3)
    print()
    key = encode([3])
    print(f"{key = }")
    t = Iterator.new_iterator(index, IteratorType.ITER_LE, key)
    while True:
        tuple_id = t.next()
        if not tuple_id:
            break
        say.say_info(f"PY | Iterator: {tuple_id = }, {res.get(tuple_id, "?")}")


def test_box():
    say.say_info(f"PY | Schema version: {box.schema_version()}")

    space: Space = box.space_by_name("test_space")
    say.say_info(f"PY | Space: {space} {type(space)}")

    index: Index = box.index_by_name(space, "primary")
    say.say_info(f"PY | Index: {index}")

    say.say_info(f"PY | Insert successful, tuple_id: {box.insert(space, encode([1, "bar"]))}")

    sleep(5) # Workaround: Possible issue with 'tx' - requires further investigation
    
    say.say_info(f"PY | Update successful, tuple_id: {box.update(index, encode([1]), encode([["=", 2, "new_value"]]))}")


def test_session():
    say.say_info(f"PY | Test Session")

    say.say_info(f"PY | Session id: {session.current()}")

    say.say_info(f"PY | Broadcast")
    session.broadcast("x", "{a = 2, [2] = 3}")


def test_index():
    say.say_info(f"PY | Test Index")

    space: Space = box.space_by_name("test_space")
    say.say_info(f"PY | Space: {space}")

    index: Index = box.index_by_name(space, "primary")
    say.say_info(f"PY | Index: {index}")

    say.say_info(f"PY | index len: {tt_index.len(index)}")
    say.say_info(f"PY | index bsize: {tt_index.bsize(index)}")
    say.say_info(f"PY | index random: {tt_index.random(index, 32)}")
    say.say_info(f"PY | index get: {tt_index.get(index, encode([1]))}")
    say.say_info(f"PY | index min: {tt_index.min(index, encode([1]))}")
    say.say_info(f"PY | index max: {tt_index.max(index, encode([1]))}")
    say.say_info(f"PY | index count: {tt_index.count(index, IteratorType.ITER_EQ ,encode([1]))}")


def test_sequence():
    say.say_info(f"PY | Test Sequence")

    seq = Sequence(id=1)
    try:
        say.say_info(f"PY | sequence.current: {sequence.current(seq)}")
    except Exception as e:
        say.say_info(f"PY | sequence.current: {str(e)}")

    say.say_info(f"PY | sequence.next: {sequence.next(seq)}")
    say.say_info(f"PY | sequence.current: {sequence.current(seq)}")
    say.say_info(f"PY | sequence.set: {sequence.set(seq, 42)}")
    say.say_info(f"PY | sequence.current: {sequence.current(seq)}")
    say.say_info(f"PY | sequence.reset: {sequence.reset(seq)}")

    try:
        sequence.current(seq)
    except Exception as e:
        say.say_info(f"PY | sequence.current: {str(e)}")


def test_key_def():
    say.say_info("PY | Test KeyDef")

    key_parts_1 = [
        KeyPart(
            field_no=0,
            field_type="unsigned",
            collation=None,
            path=None,
            flags=KeyPartFlags.IS_NULLABLE
        ),
        KeyPart(
            field_no=3,
            field_type="string",
            collation="unicode",
            path="t1",
            flags=KeyPartFlags.EXCLUDE_NULL
        ),
    ]

    key_parts_2 = [
        KeyPart(
            field_no=1,
            field_type="string",
            collation=None,
            path=None,
            flags=KeyPartFlags.EXCLUDE_NULL
        )
    ]

    t1 = key_def.new(key_parts_1)
    parts_dump_1 = key_def.dump_parts(t1)
    parts_str_1 = "\n".join("\t" + str(p) for p in parts_dump_1)
    say.say_info(f"PY | Created key_def: t1 = {t1}")
    say.say_info(f"PY | Key parts count: t1 = {key_def.part_count(t1)}")
    say.say_info(f"PY | t1 dump_parts:\n{parts_str_1}")

    t2 = key_def.new(key_parts_2)
    say.say_info(f"PY | Created key_def: t2 = {t2}")
    parts_dump_2 = key_def.dump_parts(t2)
    parts_str_2 = "\n".join("\t" + str(p) for p in parts_dump_2)
    say.say_info(f"PY | Key parts count: t2 = {key_def.part_count(t2)}")
    say.say_info(f"PY | t2 dump_parts:\n{parts_str_2}")

    t1_dup = key_def.dup(t1)
    say.say_info(f"PY | t1_dup duplicated: {t1_dup}")
    parts_dump_dup = key_def.dump_parts(t1_dup)
    parts_str_dup = "\n".join("\t" + str(p) for p in parts_dump_dup)
    say.say_info(f"PY | Created key_def: t1_dup = {t1_dup}")
    say.say_info(f"PY | Key parts count: t1_dup = {key_def.part_count(t1_dup)}")
    say.say_info(f"PY | t1_dup dump_parts:\n{parts_str_dup}")

    merged_t = key_def.merge(t1, t2)
    say.say_info(f"PY | Merged t1 and t2 → merged_t = {merged_t}")
    parts_dump_merged = key_def.dump_parts(merged_t)
    parts_str_merged = "\n".join("\t" + str(p) for p in parts_dump_merged)
    say.say_info(f"PY | Created key_def: merged_t = {merged_t}")
    say.say_info(f"PY | Key parts count: merged_t = {key_def.part_count(merged_t)}")
    say.say_info(f"PY | merged_t dump_parts:\n{parts_str_merged}")

    valid_key = encode([1, "abc"])
    invalid_key = encode(["xxx"])

    is_valid, size = key_def.validate_key(t1, valid_key)
    say.say_info(f"PY | validate_key(valid_key): {is_valid}, size = {size}")

    is_valid_full, size_full = key_def.validate_full_key(t1, valid_key)
    say.say_info(f"PY | validate_full_key(valid_key): {is_valid_full}, size = {size_full}")

    try:
        key_def.validate_key(t1, invalid_key)
    except Exception as e:
        say.say_error(f"PY | validate_key(invalid_key) error: {str(e)}")

    key_def.delete(t1)
    key_def.delete(t2)
    key_def.delete(t1_dup)
    key_def.delete(merged_t)
    say.say_info("PY | Deleted all key_defs")


def test_tuple_format():
    say.say_info("PY | Test TupleFormat")

    # Создание KeyPart-ов
    parts = [
        KeyPart(
            field_no=0,
            field_type="unsigned",
            collation=None,
            path=None,
            flags=KeyPartFlags.IS_NULLABLE
        ),
        KeyPart(
            field_no=1,
            field_type="string",
            collation=None,
            path=None,
            flags=KeyPartFlags.EXCLUDE_NULL
        ),
    ]

    kd = key_def.new(parts)
    say.say_info(f"PY | Created key_def: {kd}")

    tf = tuple_format.new([kd])
    say.say_info(f"PY | Created tuple_format: {tf}")

    tuple_format.ref(tf)
    say.say_info("PY | Called tuple_format.ref")

    tuple_format.unref(tf)
    say.say_info("PY | Called tuple_format.unref")

    default_tf = tuple_format.default()
    say.say_info(f"PY | Default tuple_format: {default_tf}")


def test_box_tuple():
    say.say_info("PY | Start box-tuple test")

    tf = tuple_format.default()
    say.say_info(f"PY | Default format: {tf}")

    data = encode([1, "abc", {"x": 42}])
    tup = tt_tuple.new(tf, data)
    say.say_info(f"PY | Created tuple: {tup}")

    rc = tt_tuple.ref(tup)
    say.say_info(f"PY | tuple.ref() => {rc}")

    fc = tt_tuple.field_count(tup)
    say.say_info(f"PY | field_count = {fc}")

    size = tt_tuple.bsize(tup)
    say.say_info(f"PY | bsize = {size}")

    buf = tt_tuple.to_buf(tup)
    say.say_info(f"PY | to_buf = {decode(buf)}")

    tf2 = tt_tuple.format(tup)
    say.say_info(f"PY | format = {tf2}")

    for i in range(fc):
        field = tt_tuple.field(tup, i)
        if isinstance(field, int):
            say.say_info(f"PY | field[{i}] = {field}")
        else:
            say.say_info(f"PY | field[{i}] error: {field}")

    path_result = tt_tuple.field_by_path(tup, "[3].x", 1)
    if isinstance(path_result, int):
        say.say_info(f"PY | field_by_path = {path_result}")
    else:
        say.say_info(f"PY | field_by_path error: {path_result}")

    update_expr = encode([["=", 1, "xyz"]])
    updated = tt_tuple.update(tup, update_expr)
    updated_val = decode(tt_tuple.to_buf(updated))
    say.say_info(f"PY | updated tuple = {updated_val}")

    upsert_expr = encode([["+", 1, 100]])
    upserted = tt_tuple.upsert(tup, upsert_expr)
    upserted_val = decode(tt_tuple.to_buf(upserted))
    say.say_info(f"PY | upserted tuple = {upserted_val}")

    valid = tt_tuple.validate(tup, tf)
    say.say_info(f"PY | validate = {valid}")

    # Create key_def to compare by first field
    key_parts = [
        KeyPart(
            field_no=0,
            field_type="unsigned",
            collation=None,
            path=None,
            flags=KeyPartFlags.EXCLUDE_NULL
        )
    ]
    kd = key_def.new(key_parts)
    say.say_info(f"PY | Created key_def: {kd}")

    # Compare with another tuple
    another_data = encode([1, "abc", {"x": 42}])
    other = tt_tuple.new(tf, another_data)
    cmp_result = tt_tuple.compare(kd, tup, other)
    say.say_info(f"PY | compare(tup, other) = {cmp_result}")

    # Compare with key
    key = encode([1])
    cmp_key_result = tt_tuple.compare_with_key(tup, key, kd)
    say.say_info(f"PY | compare_with_key(tup, [1]) = {cmp_key_result}")

    tt_tuple.unref(tup)
    say.say_info("PY | tuple.unref() done")

    say.say_info("PY | box-tuple test completed")


class Run(exports.Run):
    def run(self) -> None:
        try:
            # test_box()
            # test_session()
            # test_iterator()
            # test_index()
            # test_sequence()
            # test_key_def()
            # test_tuple_format()
            test_box_tuple()

        except Exception as e:
            say.say_error(str(e))
