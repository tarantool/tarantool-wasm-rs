from wit_world import exports
from wit_world.imports import say, sequence
from wit_world.imports.tarantool_tarantool_types import Sequence


class IncomingHandler(exports.IncomingHandler):
    def handle(self, request, response_out):
        pass


def test_sequence():
    say.say_info(f"PY | Test Sequence", None)

    seq = Sequence(id=1)
    try:
        say.say_info(f"PY | sequence.current: {sequence.current(seq)}", None)
    except Exception as e:
        say.say_info(f"PY | sequence.current: {str(e)}", None)

    say.say_info(f"PY | sequence.next: {sequence.next(seq)}", None)
    say.say_info(f"PY | sequence.current: {sequence.current(seq)}", None)
    say.say_info(f"PY | sequence.set: {sequence.set(seq, 42)}", None)
    say.say_info(f"PY | sequence.current: {sequence.current(seq)}", None)
    say.say_info(f"PY | sequence.reset: {sequence.reset(seq)}", None)

    try:
        sequence.current(seq)
    except Exception as e:
        say.say_info(f"PY | sequence.current: {str(e)}", None)


class Run(exports.Run):
    def run(self) -> None:
        try:
            test_sequence()
        except Exception as e:
            say.say_error(str(e), None)
