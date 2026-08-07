from wit_world.imports import msgpack

class WitWorld:
    def t_encode(self, data: bytes) -> bytes:
        return msgpack.from_json(data.decode("utf-8"))

    def t_decode(self, data: bytes) -> bytes:
        return msgpack.to_json(data).encode("utf-8")
