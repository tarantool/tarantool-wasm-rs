"""The SDK is an explicitly initialized library, including on native Python."""
from pathlib import Path
import subprocess
import sys
import unittest

SDK = Path(__file__).resolve().parents[2] / "sdk/src"


class SdkApiTests(unittest.TestCase):
    def run_python(self, body):
        subprocess.run([sys.executable, "-c", f"import sys; sys.path.insert(0, {str(SDK)!r})\n" + body], check=True)

    def test_import_does_not_replace_native_modules(self):
        self.run_python('''import ssl, _ssl, asyncio
ssl_before, backend_before = ssl, _ssl
resolver_before = asyncio.BaseEventLoop.getaddrinfo
import twr_python_sdk
assert sys.modules['ssl'] is ssl_before
assert sys.modules['_ssl'] is backend_before
assert asyncio.BaseEventLoop.getaddrinfo is resolver_before
twr_python_sdk.install(ports=())
assert sys.modules['ssl'] is ssl_before
assert asyncio.BaseEventLoop.getaddrinfo is resolver_before
''')

    def test_invalid_selection_changes_no_modules_or_paths(self):
        self.run_python('''import ssl, _ssl
import twr_python_sdk
before = list(sys.path)
for ports, expected in [(('ssl', 'missing'), ValueError), ('ssl', TypeError)]:
    try:
        twr_python_sdk.install(ports=ports)
    except expected:
        pass
    else:
        raise AssertionError('invalid port selection was accepted')
    assert sys.path == before
    assert sys.modules['ssl'] is ssl
    assert sys.modules['_ssl'] is _ssl
''')


if __name__ == "__main__":
    unittest.main()
