import traceback

import msgpack

from wit_world import Err, exports
from counters import Counter, add_to_latest, latest_value

_globals = {
    "__builtins__": __builtins__,
    "add_to_latest": add_to_latest,
    "latest_value": latest_value,
}


class Counters(exports.Counters):
    def peek(self, value: Counter) -> int:
        return value.get()

    def consume(self, value: Counter) -> int:
        return value.get()

    def pair(self, first: int, second: int):
        return (Counter(first), Counter(second))


class WitWorld:
    def exec(self, code: str):
        try:
            exec(code, _globals, _globals)
        except Exception as error:
            message = "".join(
                traceback.format_exception_only(type(error), error)
            ).strip()
            raise Err(message) from None

    def call(self, func: str, args_msgpack: bytes) -> bytes:
        try:
            args = msgpack.unpackb(args_msgpack, raw=False)
            if not isinstance(args, list):
                raise ValueError("args payload must be list")
            result = _globals[func](*args)
            return msgpack.packb(result, use_bin_type=True)
        except Exception as error:
            message = "".join(
                traceback.format_exception_only(type(error), error)
            ).strip()
            raise Err(message) from None
