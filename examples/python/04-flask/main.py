from wit_world import exports
from wit_world.imports import say, ttbox as box, msgpack, box_tuple
from wit_world.imports.tarantool_tarantool_types import *
from inspect import currentframe

import time
import json
import sys

import __zlib
sys.modules["zlib"] = __zlib

import importlib.metadata
# Override `version` in `importlib.metadata` since it's not available in `componentize-py`
importlib.metadata.version = lambda _: ""
# Encode an empty string with 'idna' as WASM needs this encoding
"".encode('idna')

from flask import Flask, request, jsonify

app = Flask(__name__)
f = currentframe

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


@app.route("/")
def hello():
    return "Hello from WASM!"


@app.route('/time')
def time_handler():
    current_time = time.strftime("%Y-%m-%d %H:%M:%S", time.localtime())
    return jsonify({"time": current_time})


@app.route('/insert/<space>/<tup>')
def insert_tuple(space, tup: str):
    try:
        space_obj: Space = box.space_by_name(space)

        tuple_data = [int(x) if x.isdigit() else x for x in tup.split(',')]
        say.say_info(
            f"Try to insert {tuple_data} to space {space_obj}",
            LogContext(filename=f().f_code.co_filename, line=f().f_lineno)
        )
        inserted = box.insert(space_obj, encode(tuple_data))

        return jsonify({"message": f"Inserted tup_ptr = {hex(inserted)[2:]}"})
    except Exception as e:
        return jsonify({"error": str(e)}), 400
    

@app.route('/content/<space>')
def content(space):
    try:
        say.say_info(
            f"Get space content - space '{space}'",
            LogContext(filename=f().f_code.co_filename, line=f().f_lineno)
        )
        space_obj: Space = box.space_by_name(space)
        index_obj: Index = box.index_by_name(space_obj, "primary")
        
        t = Iterator.new_iterator(index_obj, IteratorType.ITER_ALL, encode([]))
        tuples = []
        while True:
            try:
                tup = t.next()
                tuples.append(decode(data=box_tuple.to_buf(tup)))
            except:
                break

        return jsonify({"space": space, "tuples": tuples})
    except Exception as e:
        return jsonify({"error": str(e)}), 400


class IncomingHandler(exports.IncomingHandler):
    def handle(self, request, response_out):
        pass


class Run(exports.Run):
    def run(self) -> None:
        try:
            app.run(debug=False, threaded=False)
        except Exception as e:
            say.say_error(str(e.with_traceback()), None)
