from wit_world.imports import zlib as wasm_zlib

DEFLATED: int = 8
DEF_MEM_LEVEL: int
DEF_BUF_SIZE: int = 16384
MAX_WBITS: int
ZLIB_VERSION: str
ZLIB_RUNTIME_VERSION: str
Z_NO_COMPRESSION: int = 0
Z_PARTIAL_FLUSH: int = 1
Z_BEST_COMPRESSION: int = 9
Z_BEST_SPEED: int = 1
Z_BLOCK: int = 5
Z_DEFAULT_COMPRESSION: int = -1
Z_DEFAULT_STRATEGY: int = 0
Z_FILTERED: int = 1
Z_FINISH: int = 4
Z_FIXED: int = 4
Z_FULL_FLUSH: int = 3
Z_HUFFMAN_ONLY: int = 2
Z_NO_FLUSH: int = 0
Z_RLE: int = 3
Z_SYNC_FLUSH: int = 2
Z_TREES: int = 6

class error(Exception):
    """Custom zlib error for WASM-backed implementation."""
    pass

def compress(data, level=9):
    return wasm_zlib.compress(data, level)

def decompress(data):
    return wasm_zlib.decompress(data)

def crc32(data):
    return wasm_zlib.crc32(data)

def adler32(data):
    return wasm_zlib.adler32(data)
