import twr_python_sdk
twr_python_sdk.install()
from twr_python_sdk import tarantool
import _tarantool
from _tarantool import lua_callback as wit_lua_callback

from wit_world.imports import (
    box_tuple as wit_box_tuple,
    environment as wit_environment,
    exit as wit_exit,
    index as wit_index,
    insecure as wit_insecure,
    insecure_seed as wit_insecure_seed,
    instance_network as wit_instance_network,
    ip_name_lookup as wit_ip_name_lookup,
    key_def as wit_key_def,
    metrics as wit_metrics,
    monotonic_clock as wit_monotonic_clock,
    msgpack as wit_msgpack,
    network as wit_network,
    outgoing_handler as wit_outgoing_handler,
    poll as wit_poll,
    preopens as wit_preopens,
    random as wit_random,
    log as wit_log,
    sequence as wit_sequence,
    session as wit_session,
    stderr as wit_stderr,
    stdin as wit_stdin,
    stdout as wit_stdout,
    streams as wit_streams,
    tarantool_tarantool_error as wit_tarantool_tarantool_error,
    tarantool_tarantool_types as wit_tarantool_tarantool_types,
    tcp as wit_tcp,
    tcp_create_socket as wit_tcp_create_socket,
    terminal_input as wit_terminal_input,
    terminal_output as wit_terminal_output,
    terminal_stderr as wit_terminal_stderr,
    terminal_stdin as wit_terminal_stdin,
    terminal_stdout as wit_terminal_stdout,
    database as wit_database,
    tuple_format as wit_tuple_format,
    transaction as wit_transaction,
    udp as wit_udp,
    udp_create_socket as wit_udp_create_socket,
    wall_clock as wit_wall_clock,
    wasi_filesystem_types as wit_wasi_filesystem_types,
    wasi_http_types as wit_wasi_http_types,
    wasi_io_error as wit_wasi_io_error,
    wasi_tls_types as wit_wasi_tls_types,
    zlib as wit_zlib,
)
from wit_world import Err



# Freeze stdlib HTTP clients and asyncio's TLS transport into the component.
import http.client
import http.cookiejar
import http.cookies
import urllib.request
import asyncio.sslproto


import json
import base64
import binascii
import struct
import array

import re

import datetime
import time
import uuid

import collections
import functools
import itertools
import operator
import enum

import traceback

import typing
import dataclasses

import math
import hashlib
import hmac
import logging
import threading
import queue
import weakref
import mimetypes
import random
import zlib
import unicodedata
import importlib
import encodings
import pkgutil
# Requests/charset-normalizer select text codecs dynamically at runtime.
# Freeze all portable text modules, including native multibyte codecs and
# codecs without aliases such as utf_8_sig.
for _codec in pkgutil.iter_modules(encodings.__path__):
    if _codec.name not in {"mbcs", "oem", "undefined", "aliases"} and not _codec.name.endswith("_codec"):
        importlib.import_module("encodings." + _codec.name)
del _codec

from twr_python_sdk import freeze_packages
# Compile package sources into the snapshot; execute their imports at runtime.
freeze_packages("urllib3", "requests", "certifi", "idna", "charset_normalizer")

import importlib.metadata
import importlib.resources._adapters
import msgpack

_globals = {
    "__builtins__": __builtins__,
}

def _default_for_msgpack(obj):
    if isinstance(obj, datetime.datetime):
        if obj.tzinfo is None:
            obj = obj.replace(tzinfo=datetime.timezone.utc)
        return msgpack.Timestamp.from_datetime(obj)
    raise TypeError(f"Cannot serialize {type(obj)!r}")

class WitWorld:
    def exec(self, code: str):
        try:
            exec(code, _globals, _globals)
        except Exception as error:
            message = "".join(traceback.format_exception_only(type(error), error)).strip()
            raise Err(message) from None

    # componentize-py maps WIT list<u8> to Python bytes.
    def call(self, func: str, args_msgpack: bytes) -> bytes:
        try:
            args = msgpack.unpackb(args_msgpack, raw=False)
            if not isinstance(args, list):
                raise ValueError("args payload must be list")

            fn = _globals[func]
            result = fn(*args)
            return msgpack.packb(
                result,
                use_bin_type=True,
                default=_default_for_msgpack,
                datetime=True,
            )
        except Exception as error:
            message = "".join(traceback.format_exception_only(type(error), error)).strip()
            raise Err(message) from None
