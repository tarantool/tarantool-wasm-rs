from adder import exports
from adder.imports import say, sequence
from adder.imports.docs_adder_types import Sequence

def test_sequence():
    say.say_info(f"PY | Test Sequence")

    seq = Sequence(id=1)
    try:
        say.say_info(f"PY | sequence.current: {sequence.current(seq)}")
    except Exception as e:
        say.say_info(f"PY | sequence.current: {str(e)}")

    say.say_info(f"PY | sequence.next: {sequence.next(seq)}")
    say.say_info(f"PY | sequence.current: {sequence.current(seq)}")
    say.say_info(f"PY | sequence.set: {sequence.set(seq, 42)}")
    say.say_info(f"PY | sequence.current: {sequence.current(seq)}")
    say.say_info(f"PY | sequence.reset: {sequence.reset(seq)}")

    try:
        sequence.current(seq)
    except Exception as e:
        say.say_info(f"PY | sequence.current: {str(e)}")


class Run(exports.Run):
    def run(self) -> None:
        try:
            test_sequence()
        except Exception as e:
            say.say_error(str(e))
