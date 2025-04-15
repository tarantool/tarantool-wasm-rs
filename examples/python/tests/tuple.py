from adder import exports
from adder.imports import say, tarantool_tarantool_error as error, key_def, msgpack, tuple as tt_tuple, tuple_format
from adder.imports.tarantool_tarantool_types import BoxError, KeyPart, KeyPartFlags
import json

def encode(obj):
    try:
        return msgpack.encode(json.dumps(obj).encode('utf-8'))
    except Exception as error:
        say.say_error(f"PY | Error in encode: {json.dumps({'error': str(error)}, indent=2)}")
        raise


def decode(*, data = None, ptr = None):
    try:
        if ptr:
            decoded_json_bytes: bytes = msgpack.decode_from_raw_ptr(ptr)
        elif data:
            decoded_json_bytes: bytes = msgpack.decode(data)
        decoded_json_string = decoded_json_bytes.decode('utf-8')
        return json.loads(decoded_json_string)
    except Exception as error:
        say.say_error(f"PY | Error in decode: {json.dumps({'error': str(error)}, indent=2)}")
        raise

def test_box_tuple():
    say.say_info("PY | Start box-tuple test")

    tf = tuple_format.default()
    say.say_info(f"PY | Default format (ptr): {hex(tf)}")

    data = encode([1, "abc", {"x": 42}])
    tup = tt_tuple.new(data)
    say.say_info(f"PY | Created tuple {str([1, "abc", {"x": 42}])}, (ptr): {hex(tup)}")

    rc = tt_tuple.ref(tup)
    say.say_info(f"PY | tuple.ref() => {rc}")

    fc = tt_tuple.field_count(tup)
    say.say_info(f"PY | field_count = {fc}")

    size = tt_tuple.bsize(tup)
    say.say_info(f"PY | bsize = {size}")

    buf = tt_tuple.to_buf(tup)
    say.say_info(f"PY | to_buf (decoded) = {decode(data=buf)}")

    tf2 = tt_tuple.format(tup)
    say.say_info(f"PY | format (ptr) = {hex(tf2)}")

    for i in range(fc):
        field = tt_tuple.field(tup, i)
        if isinstance(field, int):
            say.say_info(f"PY | field[{i}] = {decode(ptr=field)}")
        else:
            say.say_info(f"PY | field[{i}] error: {field}")

    path_result = tt_tuple.field_by_path(tup, "[3].x", 1)
    if isinstance(path_result, int):
        say.say_info(f"PY | field_by_path \"[3].x\" = {decode(ptr=path_result)}")
    else:
        say.say_info(f"PY | field_by_path error: {path_result}")

    update_expr = encode([["=", 1, "xyz"]])
    updated = tt_tuple.update(tup, update_expr)
    updated_val = decode(data=tt_tuple.to_buf(updated))
    say.say_info(f"PY | updated tuple (tuple.update([[\"=\", 1, \"xyz\"]])) = {updated_val}")

    upsert_expr = encode([["+", 1, 100]])
    upserted = tt_tuple.upsert(tup, upsert_expr)
    upserted_val = decode(data=tt_tuple.to_buf(upserted))
    say.say_info(f"PY | upserted tuple (tuple.upsert([[\"+\", 1, 100]])) = {upserted_val}")

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
    other = tt_tuple.new(another_data)
    cmp_result = tt_tuple.compare(kd, tup, other)
    say.say_info(f"PY | tuple = {decode(data=tt_tuple.to_buf(tup))}, other = {decode(data=tt_tuple.to_buf(other))}")
    say.say_info(f"PY | compare(tup, other) = {cmp_result}")

    # Compare with key
    key = encode([1])
    cmp_key_result = tt_tuple.compare_with_key(tup, key, kd)
    say.say_info(f"PY | compare_with_key(tup, [1]) = {cmp_key_result}")

    tt_tuple.unref(tup)
    say.say_info("PY | tuple.unref() done")

    say.say_info("PY | box-tuple test completed")

def test_tuple_iterator():
    say.say_info("PY | Start box-tuple-iterator test")

    data = encode([1, "abc", {"x": 42}])
    tup = tt_tuple.new(data)
    say.say_info(f"PY | Created tuple {[1, 'abc', {'x': 42}]}, (ptr): {hex(tup)}")

    titer = tt_tuple.TupleIterator.new_tuple_iterator(tup)
    say.say_info("PY | Create tuple iterator")

    while True:
        field = titer.next()
        if not field:
            break
        say.say_info(f"PY | Iterator pos = {titer.position()}: {decode(ptr=field)}")

    pos = titer.position()
    say.say_info(f"PY | Position after full iteration: {pos}")

    titer.rewind()
    say.say_info("PY | Rewound iterator")

    field = titer.next()
    say.say_info(f"PY | call next, pos: {titer.position()}")
    say.say_info(f"PY | After rewind, first field: {decode(ptr=field)}")

    res = titer.seek(1)
    say.say_info(f"PY | call seek(1), pos: {titer.position()}")

    field = titer.next()
    say.say_info(f"PY | Field at position {titer.position()}: {decode(ptr=field)}")

class Run(exports.Run):
    def run(self) -> None:
        try:
            test_box_tuple()
            test_tuple_iterator()
        except Exception as e:
            say.say_error(str(e))
