from wit_world.imports import lua_callback


class WitWorld:
    def invoke(self, name: str, payload: bytes) -> bytes:
        return lua_callback.call(name, payload)
