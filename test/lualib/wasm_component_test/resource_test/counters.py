from wit_world import exports

_latest = None


class Counter(exports.counters.Counter):
    def __init__(self, initial: int) -> None:
        global _latest
        self.value = initial
        _latest = self

    def get(self) -> int:
        return self.value

    def add(self, delta: int) -> int:
        self.value += delta
        return self.value

    @staticmethod
    def sum(first, second) -> int:
        return first.get() + second.get()


def latest_value() -> int:
    if _latest is None:
        raise RuntimeError("no counter has been created")
    return _latest.value


def add_to_latest(delta: int) -> int:
    if _latest is None:
        raise RuntimeError("no counter has been created")
    _latest.value += delta
    return _latest.value
