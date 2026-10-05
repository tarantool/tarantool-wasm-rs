"""Run the complete pinned CPython SSL suite with its original helpers."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

from generate import ROOT, selected_module

RUNNER = '''import contextlib
import json
import sys
import unittest
from test import support
support.verbose = 0
support.use_resources = []
from test import test_ssl

class Result(unittest.TestResult):
    def __init__(self):
        super().__init__()
        self.passed = []
    def addSuccess(self, test):
        super().addSuccess(test)
        self.passed.append(test.id())

suite = unittest.defaultTestLoader.loadTestsFromModule(test_ssl)
result = Result()
with contextlib.redirect_stdout(sys.stderr):
    suite.run(result)
report = {
    'cpython_version': '3.12.11',
    'runtime_python': sys.version.split()[0],
    'openssl_version': test_ssl.ssl.OPENSSL_VERSION,
    'source_sha256': SOURCE_SHA256,
    'tests_run': result.testsRun,
    'passed': result.passed,
    'failures': [(test.id(), text) for test, text in result.failures],
    'errors': [(test.id(), text) for test, text in result.errors],
    'skipped': [(test.id(), reason) for test, reason in result.skipped],
    'expected_failures': [(test.id(), text) for test, text in result.expectedFailures],
    'unexpected_successes': [test.id() for test in result.unexpectedSuccesses],
    'resources': [],
}
print(json.dumps(report, indent=2))
sys.exit(0 if result.wasSuccessful() and result.testsRun == 183 else 1)
'''


def main():
    selection = json.loads((ROOT / 'selection.json').read_text())
    if sys.version.split()[0] != selection['cpython_version']:
        raise RuntimeError('native reference requires CPython ' + selection['cpython_version'])
    selected_module()  # Verify source, license, support and fixture checksums.
    upstream = ROOT / 'upstream' / ('cpython-' + selection['cpython_version'])
    source = upstream / 'test_ssl.py'
    with tempfile.TemporaryDirectory(prefix='ssl-native-') as directory:
        target = Path(directory)
        shutil.copytree(upstream / 'test', target / 'test')
        shutil.copytree(upstream / 'certdata', target / 'test/certdata')
        shutil.copyfile(source, target / 'test/test_ssl.py')
        runner = target / 'run.py'
        runner.write_text('SOURCE_SHA256 = ' + repr(hashlib.sha256(source.read_bytes()).hexdigest()) + '\n' + RUNNER)
        return subprocess.call([sys.executable, str(runner)], cwd=directory)


if __name__ == '__main__':
    sys.exit(main())
