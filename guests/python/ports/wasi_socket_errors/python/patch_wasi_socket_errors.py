"""Repair the pinned componentize-py 0.25.0 wasi-libc errno mapper.

The bundled mapper aborts for WASI NotInProgress/InvalidState (7/9).
Upstream wasi-libc maps both to ENOTCONN (WASI errno 53). In particular,
Linux getpeername after a TCP reset can otherwise abort during socket repr
or ResourceWarning formatting. Keep the toolchain and all other mappings.

Only the exact exported function body and (i32 pointer)->i32 signature are
accepted. Run after stripping, so changing code offsets cannot invalidate
debug/relocation sections. No imports, function indices or types change.
"""
import hashlib
import os
from pathlib import Path
import stat
import sys
import tempfile

CORE = b'\x00asm\x01\x00\x00\x00'
COMPONENT = b'\x00asm\x0d\x00\x01\x00'
EXPORT = b'__wasilibc_map_socket_error'
ORIGINAL_SHA = '7b7f9f86dab987067eed7413a74c338257ab3aa40de7d0f7a0854808c4a0b508'
PATCHED_SHA = '6529e423012b60a52b8f371b09ef54e411106107ebf79fb77c4bae627ce51bc8'
# Read the enum byte at parameter 0; it is a pointer, not the error value.
PREFIX = bytes.fromhex('20002d000041074620002d000041094672044041350f0b')


class Reader:
    def __init__(self, data):
        self.data, self.offset = data, 0

    def take(self, size):
        end = self.offset + size
        if end > len(self.data):
            raise ValueError('truncated Wasm binary')
        value = self.data[self.offset:end]
        self.offset = end
        return value

    def u32(self):
        value = 0
        for shift in range(0, 35, 7):
            byte = self.take(1)[0]
            if shift == 28 and byte > 15:
                raise ValueError('overflowing Wasm u32')
            value |= (byte & 127) << shift
            if byte < 128:
                return value
        raise ValueError('invalid Wasm u32')

    def blob(self):
        return self.take(self.u32())

    def end(self):
        if self.offset != len(self.data):
            raise ValueError('unexpected bytes in Wasm section')


def u32(value):
    result = bytearray()
    while value >= 128:
        result.append((value & 127) | 128)
        value >>= 7
    result.append(value)
    return bytes(result)


def sections(binary):
    reader = Reader(binary[8:])
    result = []
    while reader.offset < len(reader.data):
        kind = reader.take(1)[0]
        result.append((kind, reader.blob()))
    return result


def patch_module(items):
    exports = Reader(next((data for kind, data in items if kind == 7), b'\x00'))
    targets = []
    for _ in range(exports.u32()):
        name, kind, index = exports.blob(), exports.take(1)[0], exports.u32()
        if name == EXPORT:
            if kind != 0:
                raise ValueError('errno mapper must be a function export')
            targets.append(index)
    exports.end()
    if not targets:
        return items, 0
    if len(targets) != 1:
        raise ValueError('duplicate errno mapper export')
    by_kind = dict(items)
    if len([kind for kind, _ in items if kind]) != len({kind for kind, _ in items if kind}):
        raise ValueError('duplicate core Wasm section')
    if any(kind == 0 for kind, _ in items):
        raise ValueError('strip all custom sections before patching the errno mapper')

    imports = Reader(by_kind.get(2, b'\x00'))
    import_count = 0
    for _ in range(imports.u32()):
        imports.blob()
        imports.blob()
        kind = imports.take(1)[0]
        if kind == 0:
            imports.u32()
            import_count += 1
        elif kind in (1, 2):
            if kind == 1 and imports.take(1) not in (b'\x70', b'\x6f'):
                raise ValueError('unsupported libc table reference type')
            limits = imports.u32()
            if limits > (1 if kind == 1 else 3):
                raise ValueError('unsupported libc limits encoding')
            imports.u32()
            if limits & 1:
                imports.u32()
        elif kind == 3:
            if imports.take(1) not in (b'\x7f', b'\x7e', b'\x7d', b'\x7c'):
                raise ValueError('unsupported libc global type')
            if imports.take(1) not in (b'\x00', b'\x01'):
                raise ValueError('invalid libc global mutability')
        else:
            raise ValueError('unsupported libc import kind')
    imports.end()
    defined_index = targets[0] - import_count
    functions = Reader(by_kind[3])
    function_types = [functions.u32() for _ in range(functions.u32())]
    functions.end()
    if not 0 <= defined_index < len(function_types):
        raise ValueError('errno mapper must be a defined function')

    types = Reader(by_kind[1])
    type_count = types.u32()
    target_type = function_types[defined_index]
    if target_type >= type_count:
        raise ValueError('missing errno mapper type')
    for index in range(target_type + 1):
        if types.take(1) != b'\x60':
            raise ValueError('unsupported libc type encoding')
        params, results = types.take(types.u32()), types.take(types.u32())
        if index == target_type and (params, results) != (b'\x7f', b'\x7f'):
            raise ValueError('errno mapper signature must be (i32)->i32')

    code = Reader(by_kind[10])
    code_count = code.u32()
    if code_count != len(function_types):
        raise ValueError('function/code counts differ')
    bodies = [code.blob() for _ in range(code_count)]
    code.end()
    body = bodies[defined_index]
    digest = hashlib.sha256(body).hexdigest()
    if digest == ORIGINAL_SHA:
        assert body[:3] == b'\x01\x01\x7f'  # one i32 local
        bodies[defined_index] = body[:3] + PREFIX + body[3:]
        assert hashlib.sha256(bodies[defined_index]).hexdigest() == PATCHED_SHA
    elif digest != PATCHED_SHA:
        raise ValueError('unsupported pinned wasi-libc errno mapper: ' + digest)
    payload = u32(code_count) + b''.join(u32(len(body)) + body for body in bodies)
    return [(kind, payload if kind == 10 else data) for kind, data in items], 1


def patch(binary):
    header = binary[:8]
    if header not in (CORE, COMPONENT):
        raise ValueError('expected a core Wasm module or component')
    items = sections(binary)
    count = 0
    if header == CORE:
        items, count = patch_module(items)
    else:
        nested = []
        for kind, data in items:
            if kind in (1, 4):
                data, matches = patch(data)
                count += matches
            nested.append((kind, data))
        items = nested
    return header + b''.join(bytes([kind]) + u32(len(data)) + data for kind, data in items), count


def patch_file(path):
    path = Path(path)
    result, count = patch(path.read_bytes())
    if count != 1:
        raise ValueError(f'expected exactly one pinned errno mapper, found {count}')
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent, prefix=path.name + '.', delete=False) as file:
            temporary = Path(file.name)
            file.write(result)
        temporary.chmod(stat.S_IMODE(path.stat().st_mode))
        os.replace(temporary, path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


if __name__ == '__main__':
    if len(sys.argv) != 2:
        raise SystemExit('usage: patch_wasi_socket_errors.py COMPONENT.wasm')
    patch_file(sys.argv[1])
