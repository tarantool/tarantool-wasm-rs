from adder import exports
from adder.imports import say, msgpack, session, txn
from adder.imports import index as tt_index
from adder.imports import ttbox as box
from adder.imports.docs_adder_types import Space, Index, TxnIsolationLevel, Iterator, IteratorType
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


class Run(exports.Run):
    def run(self) -> None:
        try:
            # test_box()
            # test_session()
            # test_iterator()
            test_index()

        except Exception as e:
            say.say_error(str(e))
