"""Errors retain the information returned by the host."""


class TarantoolError(Exception):
    def __init__(self, code, message, error_type="", file=None, line=None):
        super().__init__(message)
        self.code = code
        self.message = message
        self.error_type = error_type
        self.file = file
        self.line = line

    @classmethod
    def from_record(cls, value):
        return cls(value.code, value.message, value.error_type, value.file, value.line)


class LuaCallbackError(Exception):
    """The runtime rejected a Lua callback or its invocation failed."""


class ClosedResourceError(ValueError):
    """An SDK resource has already been closed."""
