from adder import exports
from adder.imports import say, docs_adder_error as error
from adder.imports.docs_adder_types import BoxError


def test_error():
    say.say_info("PY | Running test_error")

    last = error.last()
    if last:
        say.say_info(f"PY | Existing error: {error.to_string(last)}")
    else:
        say.say_info("PY | No existing error found")

    err: BoxError = error.error_new("Test", "from python", 228)
    say.say_info(f"PY | Created error: {error.to_string(err)}")

    error.set(err)
    say.say_info("PY | Error set")

    last_after_set = error.last()
    if last_after_set:
        say.say_info(f"PY | Error after set: {error.to_string(last_after_set)}")
    else:
        say.say_warn("PY | No error found after set() call!")
    
    error.clear()

    last = error.last()
    if last:
        say.say_info(f"PY | Existing error: {error.to_string(last)}")
    else:
        say.say_info("PY | No existing error found")

    say.say_info("PY | test_error completed")


class Run(exports.Run):
    def run(self) -> None:
        try:
            test_error()
        except Exception as e:
            say.say_error(str(e))
