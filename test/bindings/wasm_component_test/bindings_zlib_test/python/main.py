from wit_world.imports import zlib

class WitWorld:

    def t_compress(self, data: bytes, level: int) -> bytes:
        return zlib.compress(data, level)

    def t_decompress(self, data: bytes) -> bytes:
        return zlib.decompress(data)

    def t_crc32(self, data: bytes) -> int:
        return zlib.crc32(data)

    def t_adler32(self, data: bytes) -> int:
        return zlib.adler32(data)
