"""Keep generated WIT names out of the public API."""

import _tarantool


class Bindings:
    def __init__(self, modules=None):
        self._modules = dict(modules or {})

    def module(self, name):
        if name not in self._modules:
            self._modules[name] = getattr(_tarantool, name)
        return self._modules[name]

    def call(self, interface, method, *args):
        return self.invoke(getattr(self.module(interface), method), *args)

    @staticmethod
    def invoke(function, *args):
        from componentize_py_types import Err
        from .errors import TarantoolError, LuaCallbackError

        try:
            return function(*args)
        except Err as error:
            value = error.value
            if isinstance(value, str):
                raise LuaCallbackError(value) from error
            raise TarantoolError.from_record(value) from error

    def record(self, name, *args):
        return getattr(self.module("types"), name)(*args)

    def enum(self, name, value):
        enum = getattr(self.module("types"), name)
        return enum[value.name]
