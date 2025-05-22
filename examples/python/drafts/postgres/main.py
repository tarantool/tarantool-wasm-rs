import tlslite.errors
from adder import exports
from adder.imports import say, ttbox as box, msgpack, tuple as tt_tuple
from adder.imports.tarantool_tarantool_types import *
from inspect import currentframe

class IncomingHandler(exports.IncomingHandler):
    def handle(self, request, response_out):
        print("HTTP HANDLE")

import json
import sys

from inner import zlib
sys.modules["zlib"] = zlib

# Encode an empty string with 'idna' as WASM needs this encoding
"".encode('idna')

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

import socket

saved_getaddrinfo = socket.getaddrinfo


def _getaddrinfo(host, port, *args, **kwargs):
    res = saved_getaddrinfo(host, port, *args, **kwargs)
    updated_res = []
    for item in res:
        family, socktype, proto, canonname, sockaddr = item

        if isinstance(sockaddr, tuple) and len(sockaddr) == 2:  # IPv4: ('127.0.0.1', 9092)
            updated_sockaddr = (sockaddr[0], port)
        elif isinstance(sockaddr, tuple) and len(sockaddr) == 4:  # IPv6: ('::1', 9092, 0, 0)
            updated_sockaddr = (sockaddr[0], port, sockaddr[2], sockaddr[3])
        elif isinstance(sockaddr, tuple) and len(sockaddr) == 1:  # Other formats: (IPv4 address)
            updated_sockaddr = (sockaddr[0], port)
        else:
            updated_sockaddr = sockaddr  # If it's already in expected format

        updated_res.append((family, socktype, proto, canonname, updated_sockaddr))

    return updated_res

socket.getaddrinfo = _getaddrinfo

import tlslite

# import asyncio
# import asyncpg

# async def connect():
#     conn = await asyncpg.connect(
#         user='postgres',
#         password='mysecretpassword',
#         database='mydatabase',
#         host='127.0.0.1',
#         port='5432'
#     )
#     print("Connected to the database.")
#     await conn.close()

class Run(exports.Run):
    def run(self) -> None:
        try:
            pass
            # loop = asyncio.get_event_loop()
            # loop.run_until_complete(connect())
        except Exception as e:
            say.say_error(str(e.with_traceback()), None)
