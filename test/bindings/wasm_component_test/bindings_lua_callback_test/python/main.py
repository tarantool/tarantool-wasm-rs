from wit_world.imports import lua_callback

_handlers = {}


class WitWorld:
    def acquire(self, name: str) -> None:
        _handlers[name] = lua_callback.open(name)

    def invoke(self, name: str, payload: bytes) -> bytes:
        handler = _handlers.get(name)
        if handler is None:
            handler = lua_callback.open(name)
        return handler.call(payload)

    def release(self, name: str) -> None:
        _handlers.pop(name, None)
