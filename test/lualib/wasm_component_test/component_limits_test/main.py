class WitWorld:

    def foo(self) -> None:
        c = 0
        while True:
            if c == 10000000:
                break
            c += 1

    def bar(self) -> None:
        a = []
        while True:
            a.append(1)
            c = len(a)
            if c == 10000000:
                break
