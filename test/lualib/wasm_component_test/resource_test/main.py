from wit_world import exports
from counters import Counter


class Counters(exports.Counters):
    def peek(self, value: Counter) -> int:
        return value.get()

    def consume(self, value: Counter) -> int:
        return value.get()

    def pair(self, first: int, second: int):
        return (Counter(first), Counter(second))
