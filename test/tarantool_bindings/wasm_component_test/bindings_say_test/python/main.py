from typing import Optional

from wit_world.imports import log, types


class WitWorld:
    def t_say(
        self,
        level: types.LogLevel,
        msg: str,
        ctx: Optional[types.LogContext],
    ) -> None:
        log.write(level, msg, ctx)

    def t_say_error(
        self, msg: str, ctx: Optional[types.LogContext]
    ) -> None:
        log.write(types.LogLevel.ERROR, msg, ctx)

    def t_say_crit(
        self, msg: str, ctx: Optional[types.LogContext]
    ) -> None:
        log.write(types.LogLevel.CRITICAL, msg, ctx)

    def t_say_warn(
        self, msg: str, ctx: Optional[types.LogContext]
    ) -> None:
        log.write(types.LogLevel.WARNING, msg, ctx)

    def t_say_info(
        self, msg: str, ctx: Optional[types.LogContext]
    ) -> None:
        log.write(types.LogLevel.INFO, msg, ctx)

    def t_say_verbose(
        self, msg: str, ctx: Optional[types.LogContext]
    ) -> None:
        log.write(types.LogLevel.VERBOSE, msg, ctx)

    def t_say_debug(
        self, msg: str, ctx: Optional[types.LogContext]
    ) -> None:
        log.write(types.LogLevel.DEBUG, msg, ctx)
