from wit_world.imports import msgpack

class WitWorld:
    def t_encode(self, data: bytes) -> bytes:
        return msgpack.encode(data)

    def t_decode(self, data: bytes) -> bytes:
        return msgpack.decode(data)

    def t_decode_from_raw_ptr(self, ptr: int) -> bytes:
        return msgpack.decode_from_raw_ptr(ptr)
