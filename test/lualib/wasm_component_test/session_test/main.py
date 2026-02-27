count = None

def init():
    global count
    if count is None:
        count = 0

def inc():
    global count
    count += 1
    return count

class WitWorld:
    def t_func(self) -> int:
        init()
        return inc()
