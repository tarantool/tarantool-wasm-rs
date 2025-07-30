from wit_world.imports import say, types
from typing import Optional

class WitWorld:

    def t_say(self, level: types.LogLevel, msg: str, ctx: Optional[types.LogContext]) -> None:
        say.say(level, msg, ctx)

    def t_say_error(self, msg: str, ctx: Optional[types.LogContext]) -> None:
        say.say_error(msg, ctx)

    def t_say_crit(self, msg: str, ctx: Optional[types.LogContext]) -> None:
        say.say_crit(msg, ctx)

    def t_say_warn(self, msg: str, ctx: Optional[types.LogContext]) -> None:
        say.say_warn(msg, ctx)

    def t_say_info(self, msg: str, ctx: Optional[types.LogContext]) -> None:
        say.say_info(msg, ctx)

    def t_say_verbose(self, msg: str, ctx: Optional[types.LogContext]) -> None:
        say.say_verbose(msg, ctx)

    def t_say_debug(self, msg: str, ctx: Optional[types.LogContext]) -> None:
        say.say_debug(msg, ctx)

    def t_say_syserror(self, msg: str, ctx: Optional[types.LogContext]) -> None:
        say.say_syserror(msg, ctx)
