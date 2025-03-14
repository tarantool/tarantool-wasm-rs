from adder import exports
from adder.imports import say, msgpack
from adder.imports import ttbox as box
from adder.imports.docs_adder_types import Space, Index
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


class Run(exports.Run):
    def run(self) -> None:
        try:
            say.say_info(f"PY | Schema version: {box.schema_version()}")

            space: Space = box.space_by_name("test_space")
            say.say_info(f"PY | Space: {space} {type(space)}")

            index: Index = box.index_by_name(space, "primary")
            say.say_info(f"PY | Index: {index}")

            say.say_info(f"PY | Insert successful, tuple_id: {box.insert(space, encode([1, "bar"]))}")

            sleep(5) # Workaround: Possible issue with 'tx' - requires further investigation
            
            say.say_info(f"PY | Update successful, tuple_id: {box.update(index, encode([1]), encode([["=", 2, "new_value"]]))}")

        except Exception as e:
            say.say_error(str(e))
