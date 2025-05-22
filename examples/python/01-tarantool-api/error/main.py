from wit_world import exports
from wit_world.imports import say, tarantool_tarantool_error as error
from wit_world.imports.tarantool_tarantool_types import BoxError, LogContext
from inspect import currentframe

class IncomingHandler(exports.IncomingHandler):
    def handle(self, request, response_out):
        pass

f = currentframe

def test_error():
    say.say_info("PY | Running test_error", LogContext(filename=f().f_code.co_filename, line=f().f_lineno))

    last = error.last()
    if last:
        say.say_info(f"PY | Existing error: {error.to_string(last)}", None)
    else:
        say.say_info("PY | No existing error found", None)

    err: BoxError = error.error_new("Test", "from python", 228, f().f_code.co_filename, f().f_lineno )
    say.say_info(f"PY | Created error: {error.to_string(err)}", None)

    error.set(err)
    say.say_info("PY | Error set", None)

    last_after_set = error.last()
    if last_after_set:
        say.say_info(f"PY | Error after set: {error.to_string(last_after_set)}", None)
    else:
        say.say_warn("PY | No error found after set() call!", None)
    
    error.clear()

    last = error.last()
    if last:
        say.say_info(f"PY | Existing error: {error.to_string(last)}", None)
    else:
        say.say_info("PY | No existing error found", None)

    say.say_info("PY | test_error completed", LogContext(filename=f().f_code.co_filename, line=f().f_lineno))


class Run(exports.Run):
    def run(self) -> None:
        try:
            test_error()
        except Exception as e:
            say.say_error(str(e), None)
