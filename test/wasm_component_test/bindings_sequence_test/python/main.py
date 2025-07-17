from wit_world.imports import types, sequence
from wit_world import exports

class Run(exports.Run):
    def run(self):
        seq = types.Sequence(id=1)

        try:
            current = sequence.current(seq)
            assert False, f"Expected error, got: {current}"
        except Exception as e:
            assert "is not started" in str(e), f"Unexpected error: {e}"

        next_val = sequence.next(seq)
        assert next_val == 1, f"Expected 1, got: {next_val}"

        current_val = sequence.current(seq)
        assert current_val == 1, f"Expected 1, got: {current_val}"

        next_val2 = sequence.next(seq)
        assert next_val2 == 2, f"Expected 2, got: {next_val2}"

        sequence.set(seq, 100)

        current_after_set = sequence.current(seq)
        assert current_after_set == 100, f"Expected 100, got: {current_after_set}"

        next_val3 = sequence.next(seq)
        assert next_val3 == 101, f"Expected 101, got: {next_val3}"

        next_val4 = sequence.next(seq)
        assert next_val4 == 102, f"Expected 102, got: {next_val4}"

        sequence.reset(seq)

        try:
            current_after_reset = sequence.current(seq)
            assert False, f"Expected error after reset, got: {current_after_reset}"
        except Exception as e:
            assert "is not started" in str(e), f"Unexpected error: {e}"

        next_val5 = sequence.next(seq)
        assert next_val5 == 1, f"Expected 1, got: {next_val5}"

        next_val6 = sequence.next(seq)
        assert next_val6 == 2, f"Expected 2, got: {next_val6}"
