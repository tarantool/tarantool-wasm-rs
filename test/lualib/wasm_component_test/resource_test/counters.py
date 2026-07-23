from wit_world import exports


class Counter(exports.counters.Counter):
    def __init__(self, initial: int) -> None:
        self.value = initial

    def get(self) -> int:
        return self.value

    def add(self, delta: int) -> int:
        self.value += delta
        return self.value

    @staticmethod
    def sum(first, second) -> int:
        return first.get() + second.get()
