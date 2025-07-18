from wit_world.imports import types, sequence

class WitWorld:
    def t_next(self, seq: types.Sequence) -> int:
        return sequence.next(seq)

    def t_current(self, seq: types.Sequence) -> int:
        return sequence.current(seq)

    def t_set(self, seq: types.Sequence, value: int) -> None:
        return sequence.set(seq, value)

    def t_reset(self, seq: types.Sequence) -> None:
        return sequence.reset(seq)
