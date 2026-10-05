"""Session, sequence, diagnostics, logging and Lua callback APIs."""

import logging
from . import _codec
from ._resources import WitResource
from .database import unsigned
from .errors import TarantoolError
from .types import LogLevel


class Sequence:
    def __init__(self, bindings, id):
        self._bindings = bindings
        self.id = unsigned(id)

    def _call(self, method, *args):
        return self._bindings.call(
            "sequence", method, self._bindings.record("Sequence", self.id), *args
        )

    def next(self):
        return self._call("next")

    @property
    def current(self):
        return self._call("current")

    def set(self, value):
        self._call("set", value)

    def reset(self):
        self._call("reset")


class Session:
    def __init__(self, bindings, id):
        self._bindings = bindings
        self.id = unsigned(id, 64)

    def send(self, header, body=None):
        if not isinstance(header, dict) or (
            body is not None and not isinstance(body, dict)
        ):
            raise TypeError("iproto header and body must be dictionaries")
        self._bindings.call(
            "session",
            "iproto_send",
            self._bindings.record("Session", self.id),
            _codec.pack(header),
            None if body is None else _codec.pack(body),
        )


class Sessions:
    def __init__(self, bindings):
        self._bindings = bindings

    def current(self):
        return Session(self._bindings, self._bindings.call("session", "current").id)

    def __getitem__(self, id):
        return Session(self._bindings, id)

    def broadcast_lua(self, key, expression):
        self._bindings.call("session", "broadcast", key, expression)


class Errors:
    def __init__(self, bindings):
        self._bindings = bindings

    def new(self, code, message, *, file=None, line=None):
        code = unsigned(code)
        if file is None and line is None:
            value = self._bindings.call("error", "new", code, message)
        else:
            value = self._bindings.call(
                "error",
                "new_with_location",
                code,
                message,
                file,
                None if line is None else unsigned(line),
            )
        return TarantoolError.from_record(value)

    def _record(self, error):
        if not isinstance(error, TarantoolError):
            raise TypeError("expected TarantoolError")
        return self._bindings.record(
            "BoxError",
            error.code,
            error.message,
            error.error_type,
            error.file,
            error.line,
        )

    def set(self, error):
        self._bindings.call("error", "set", self._record(error))

    def last(self):
        value = self._bindings.call("error", "last")
        return None if value is None else TarantoolError.from_record(value)

    def clear(self):
        self._bindings.call("error", "clear")

    def format(self, error):
        return self._bindings.call("error", "to_string", self._record(error))


class Log:
    def __init__(self, bindings):
        self._bindings = bindings

    def write(self, level, message, *, context=None):
        location = (
            None
            if context is None
            else self._bindings.record(
                "LogContext", context.file, unsigned(context.line)
            )
        )
        self._bindings.call(
            "log",
            "write",
            self._bindings.enum("LogLevel", LogLevel(level)),
            str(message),
            location,
        )

    def handler(self, level=logging.NOTSET):
        return TarantoolLogHandler(self, level)


class TarantoolLogHandler(logging.Handler):
    def __init__(self, log, level=logging.NOTSET):
        super().__init__(level)
        self._log = log

    def emit(self, record):
        from .types import LogContext

        if record.levelno >= logging.CRITICAL:
            level = LogLevel.CRITICAL
        elif record.levelno >= logging.ERROR:
            level = LogLevel.ERROR
        elif record.levelno >= logging.WARNING:
            level = LogLevel.WARNING
        elif record.levelno >= logging.INFO:
            level = LogLevel.INFO
        else:
            level = LogLevel.DEBUG
        try:
            self._log.write(
                level,
                self.format(record),
                context=LogContext(record.pathname, record.lineno),
            )
        except Exception:
            self.handleError(record)


class Metrics:
    def __init__(self, bindings):
        self._bindings = bindings

    def uptime(self):
        return self._bindings.call("metrics", "uptime")


class MessagePack:
    def __init__(self, bindings):
        self._bindings = bindings

    pack = staticmethod(_codec.pack)
    unpack = staticmethod(_codec.unpack)

    def from_json(self, value):
        return self._bindings.call("msgpack", "from_json", value)

    def to_json(self, value):
        return self._bindings.call("msgpack", "to_json", bytes(value))


class LuaCallback(WitResource):
    def __call__(self, payload=None):
        return _codec.unpack(
            self._bindings.invoke(self._get().call, _codec.pack(payload))
        )
