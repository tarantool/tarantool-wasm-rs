from adder import exports
from adder.imports import say, msgpack
from adder.imports import tuple as tt_tuple
from adder.imports import ttbox as box
from adder.imports.docs_adder_types import Space, Index, Iterator, IteratorType
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


def test_box():
    say.say_info(f"PY | ======================== Test box ========================")

    # schema-version
    schema_ver = box.schema_version()
    say.say_info(f"PY | Schema version: {schema_ver}")

    # space-by-name
    space: Space = box.space_by_name("test_space")
    say.say_info(f"PY | Space: {space.id = }")

    # index-by-name
    index: Index = box.index_by_name(space, "primary")
    say.say_info(f"PY | Index: {index.space_id = } {index.id = } {index.index_base = }")

    test_data = [
        [1, "Alice", 25],
        [2, "Bob", 30],
        [3, "Charlie", 35],
        [4, "David", 40],
        [5, "Eve", 4],
    ]

    # truncate
    box.truncate(space)
    say.say_info("PY | Truncated space")

    # insert
    for row in test_data:
        tup_bytes = encode(row)
        inserted = box.insert(space, tup_bytes)
        say.say_info(f"PY | Inserted: {row}, tuple ptr = {hex(inserted)}, decoded tuple from ptr {decode(data=tt_tuple.to_buf(inserted))}")

    # update (update age of id=1 to 26)
    key = encode([1])
    ops = encode([["=", 2, 26]])  # Set field at index 2 (age) to 26
    updated = box.update(index, key, ops)
    say.say_info(f"PY | Updated tuple with id=1 -> age=26, ptr = {hex(updated)}, decoded tuple from ptr {decode(data=tt_tuple.to_buf(updated))}")

    # replace (replace entire tuple for id=2)
    replaced = box.replace(space, encode([2, "Bob Jr.", 31]))
    say.say_info(f"PY | Replaced id=2 -> ['Bob Jr.', 31], ptr = {hex(replaced)}, decoded tuple from ptr {decode(data=tt_tuple.to_buf(replaced))}")

    # upsert (id=6 doesn't exist → insert)
    upsert_tuple = encode([6, "Frank", 50])
    upsert_ops = encode([["+", 2, 1]])  # Increase age if exists
    _ = box.upsert(index, upsert_tuple, upsert_ops)
    say.say_info(f"PY | Upserted id=6 (inserted)")

    # upsert (id=1 exists → increment age)
    _ = box.upsert(index, encode([1, "Alice", 0]), encode([["+", 2, 1]]))
    say.say_info(f"PY | Upserted id=1 (age += 1)")

    # tt-delete (delete id=3)
    deleted = box.tt_delete(index, encode([3]))
    say.say_info(f"PY | Deleted id=3, ptr = {hex(deleted)}, decoded tuple from ptr {decode(data=tt_tuple.to_buf(deleted))}")

    say.say_info("PY | Space content:")
    t = Iterator.new_iterator(index, IteratorType.ITER_ALL, encode([]))
    while True:
        tup = t.next()
        if not tup:
            break
        say.say_info(f"PY |\t Element: {decode(data=tt_tuple.to_buf(tup))}")

    # truncate again
    box.truncate(space)
    say.say_info("PY | Truncated space again")

    say.say_info(f"PY | ===================== Done test box ======================")


class Run(exports.Run):
    def run(self) -> None:
        try:
            test_box()
        except Exception as e:
            say.say_error(str(e))
