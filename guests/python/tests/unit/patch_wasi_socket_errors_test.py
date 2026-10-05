"""Pinned SDK patch guards, section preservation and atomic failure."""
import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest

_path = Path(__file__).resolve().parents[2] / 'ports/wasi_socket_errors/python/patch_wasi_socket_errors.py'
_spec = importlib.util.spec_from_file_location('socket_patch', _path)
patcher = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(patcher)

# Real SDK function body; the module below gives its abort call a valid target.
BODY = bytes.fromhex(
    '01017f4102210102400240024002400240024002400240024002400240024002'
    '4002400240024020002d0000417f6a0e140f0e000102030d040d05060708090a'
    '0b0c0d0d0d0e0b411c0f0b41300f0b41c9000f0b41070f0b41060f0b41210f0b'
    '41040f0b41030f0b41170f0b410e0f0b410f0f0b410d0f0b41230f0b10ef8280'
    '8000000b413a21010b20010b')


def section(kind, payload):
    return bytes([kind]) + patcher.u32(len(payload)) + payload


def blob(value):
    return patcher.u32(len(value)) + value


def fixture(body=BODY, signature=b'\x60\x01\x7f\x01\x7f'):
    types = b'\x02' + signature + b'\x60\x00\x00'
    imports = b'\x04' + b''.join(
        blob(b'env') + blob(name) + descriptor for name, descriptor in (
            (b'memory', b'\x02\x00\x01'),
            (b'table', b'\x01\x70\x00\x01'),
            (b'stack_pointer', b'\x03\x7f\x01'),
            (b'function', b'\x00\x00')))
    function_types = [0] * 370
    function_types[366] = 1  # absolute index 367: abort()->void
    functions = patcher.u32(370) + bytes(function_types)
    exports = b'\x01' + blob(patcher.EXPORT) + b'\x00' + patcher.u32(370)
    bodies = [b'\x00\x41\x00\x0b'] * 370
    bodies[366], bodies[369] = b'\x00\x00\x0b', body
    code = patcher.u32(370) + b''.join(blob(value) for value in bodies)
    return patcher.CORE + b''.join(section(kind, data) for kind, data in (
        (1, types), (2, imports), (3, functions), (7, exports), (10, code)))


class PatchTests(unittest.TestCase):
    def test_pinned_body_and_nested_sections(self):
        self.assertEqual(hashlib.sha256(BODY).hexdigest(), patcher.ORIGINAL_SHA)
        original = fixture()
        nested = patcher.COMPONENT + section(4, patcher.COMPONENT + section(1, original))
        result, count = patcher.patch(nested)
        self.assertEqual(count, 1)
        self.assertEqual(patcher.patch(result), (result, 1))
        core, count = patcher.patch(original)
        self.assertEqual(count, 1)
        self.assertEqual(patcher.sections(core)[:-1], patcher.sections(original)[:-1])
        code = patcher.Reader(patcher.sections(core)[-1][1])
        bodies = [code.blob() for _ in range(code.u32())]
        code.end()
        self.assertEqual(hashlib.sha256(bodies[-1]).hexdigest(), patcher.PATCHED_SHA)
        self.assertEqual(bodies[366], b'\x00\x00\x0b')

    def test_unknown_sdk_signature_and_custom_sections_fail(self):
        for binary in (fixture(BODY[:-1] + b'\x00'),
                       fixture(signature=b'\x60\x01\x7e\x01\x7f'),
                       fixture() + section(0, blob(b'reloc.CODE'))):
            with self.assertRaises(ValueError):
                patcher.patch(binary)

    def test_atomic_file_guards_and_permissions(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'component.wasm'
            for original in (patcher.COMPONENT,
                             patcher.COMPONENT + section(1, fixture()) * 2,
                             fixture(BODY[:-1] + b'\x00')):
                path.write_bytes(original)
                with self.assertRaises(ValueError):
                    patcher.patch_file(path)
                self.assertEqual(path.read_bytes(), original)
                self.assertEqual(list(Path(directory).iterdir()), [path])
            path.write_bytes(fixture())
            path.chmod(0o640)
            patcher.patch_file(path)
            self.assertEqual(path.stat().st_mode & 0o777, 0o640)
            patched = path.read_bytes()
            patcher.patch_file(path)
            self.assertEqual(path.read_bytes(), patched)

    def test_truncated_and_overflowing_sections_fail(self):
        for binary in (b'', patcher.CORE + b'\x01\x80',
                       patcher.CORE + b'\x01\xff\xff\xff\xff\x10',
                       patcher.COMPONENT + section(1, b'unknown')):
            with self.assertRaises(ValueError):
                patcher.patch(binary)


if __name__ == '__main__':
    unittest.main()
