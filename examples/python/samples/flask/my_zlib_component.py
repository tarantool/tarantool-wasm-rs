from adder.imports import zlib as wasm_zlib

def compress(data, level=9):
    return wasm_zlib.compress(data, level)

def decompress(data):
    return wasm_zlib.decompress(data)

def crc32(data):
    return wasm_zlib.crc32(data)

def adler32(data):
    return wasm_zlib.adler32(data)
