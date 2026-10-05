"""Python API for tarantool-wit 0.2.0 and the runtime's Lua callbacks.

Importing this module neither calls the host nor installs compatibility ports.
"""

from ._bindings import Bindings
from .database import Space, Index, Spaces
from .errors import TarantoolError, LuaCallbackError, ClosedResourceError
from .services import (
    Sequence,
    Sessions,
    Errors,
    Log,
    Metrics,
    MessagePack,
    LuaCallback,
    Session,
    TarantoolLogHandler,
)
from .transaction import Transactions
from .tuples import Tuple, KeyDef, TupleFormat, key_part
from .types import (
    Collation,
    FieldType,
    IteratorType,
    KeyPart,
    KeyPartFlags,
    LogContext,
    LogLevel,
    TransactionIsolation,
)
from . import _codec
from .values import Datetime, Interval, IntervalAdjust, ErrorFrame, ErrorValue


class Tarantool:
    def __init__(self, *, bindings=None):
        self._bindings = Bindings(bindings)
        self.space = Spaces(self._bindings)
        self.transaction = Transactions(self._bindings)
        self.session = Sessions(self._bindings)
        self.error = Errors(self._bindings)
        self.log = Log(self._bindings)
        self.metrics = Metrics(self._bindings)
        self.msgpack = MessagePack(self._bindings)

    @property
    def schema_version(self):
        return self._bindings.call("database", "schema_version")

    def sequence(self, id):
        return Sequence(self._bindings, id)

    def tuple(self, value):
        return Tuple(
            self._bindings, self._bindings.call("box_tuple", "new", _codec.array(value))
        )

    def key_def(self, parts):
        parts = [key_part(self._bindings, part) for part in parts]
        return KeyDef(self._bindings, self._bindings.call("key_def", "new", parts))

    def tuple_format(self, key_defs=None):
        if key_defs is None:
            handle = self._bindings.call("tuple_format", "default")
        else:
            handles = []
            for definition in key_defs:
                if definition._bindings is not self._bindings:
                    raise ValueError(
                        "key definition belongs to different Tarantool bindings"
                    )
                handles.append(definition._get())
            handle = self._bindings.call("tuple_format", "new", handles)
        return TupleFormat(self._bindings, handle)

    def lua_callback(self, name):
        return LuaCallback(
            self._bindings, self._bindings.call("lua_callback", "open", name)
        )


_default = Tarantool()
space = _default.space
transaction = _default.transaction
session = _default.session
error = _default.error
log = _default.log
metrics = _default.metrics
msgpack = _default.msgpack
sequence = _default.sequence
tuple = _default.tuple
key_def = _default.key_def
tuple_format = _default.tuple_format
lua_callback = _default.lua_callback


def schema_version():
    return _default.schema_version


__all__ = [
    "Tarantool",
    "Datetime",
    "Interval",
    "IntervalAdjust",
    "ErrorFrame",
    "ErrorValue",
    "Space",
    "Index",
    "Tuple",
    "KeyDef",
    "TupleFormat",
    "Sequence",
    "Session",
    "LuaCallback",
    "TarantoolError",
    "LuaCallbackError",
    "ClosedResourceError",
    "TarantoolLogHandler",
    "Collation",
    "FieldType",
    "IteratorType",
    "KeyPart",
    "KeyPartFlags",
    "LogContext",
    "LogLevel",
    "TransactionIsolation",
    "space",
    "transaction",
    "session",
    "error",
    "log",
    "metrics",
    "msgpack",
    "sequence",
    "tuple",
    "key_def",
    "tuple_format",
    "lua_callback",
    "schema_version",
]
