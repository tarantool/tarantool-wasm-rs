from wit_world.imports import math, say


class WitWorld:
    def call_chain_add(self, left: int, right: int) -> int:
        say.say_info(f"python consumer: call-chain-add({left}, {right})", None)
        return math.add(left, right)

    def call_chain_label(self, left: int, right: int) -> str:
        say.say_info(f"python consumer: call-chain-label({left}, {right})", None)
        total = math.add(left, right)
        label = math.label(left, right)
        return f"python saw [{label}], total = {total}"
