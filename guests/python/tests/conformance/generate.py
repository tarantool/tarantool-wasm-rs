"""Select unchanged upstream test methods; no test.support replacements."""
import argparse
import ast
import copy
import hashlib
import json
from pathlib import Path
import os
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent
HEADER = '''import array
import contextlib
import enum
import errno
import functools
import gc
import hashlib
import json
import os
import pprint
import re
import socket
import ssl
import _ssl
import sys
import sysconfig
import time
import traceback
import unittest
import unittest.mock
import weakref
from test import support
from test.support import import_helper, os_helper, socket_helper, warnings_helper
from ssl import TLSVersion, _TLSContentType, _TLSMessageType, _TLSAlertType
if os.environ.get('TWR_SSL_CONFORMANCE_DATA') is not None:
    __file__ = os.path.join(os.environ['TWR_SSL_CONFORMANCE_DATA'], 'test_ssl.py')
'''
RUNNER = '''
class _Result(unittest.TestResult):
    def __init__(self):
        super().__init__()
        self.passed = []

    def addSuccess(self, test):
        super().addSuccess(test)
        self.passed.append(test.id())

    def _exc_info_to_string(self, err, test):
        # Keep all diagnostics without unittest's mutation of chained
        # tracebacks; generated WIT exception dataclasses can be frozen.
        return ''.join(traceback.format_exception(*err))

def run():
    for relative, expected in FIXTURE_SHA256.items():
        with open(os.path.join(os.path.dirname(__file__), relative), 'rb') as fixture:
            if hashlib.sha256(fixture.read()).hexdigest() != expected:
                raise RuntimeError('upstream fixture checksum mismatch: ' + relative)
    suite = unittest.defaultTestLoader.loadTestsFromModule(sys.modules[__name__])
    if suite.countTestCases() != len(SELECTED):
        raise RuntimeError('upstream test selection count changed')
    result = _Result()
    with contextlib.redirect_stdout(sys.stderr):
        suite.run(result)
    return {
        'cpython_version': CPYTHON_VERSION,
        'runtime_python': sys.version.split()[0],
        'runtime_implementation': sys.implementation.name,
        'openssl_version': ssl.OPENSSL_VERSION,
        'source_sha256': SOURCE_SHA256,
        'fixture_sha256': FIXTURE_SHA256,
        'selected': SELECTED,
        'excluded': EXCLUDED,
        'tests_run': result.testsRun,
        'passed': result.passed,
        'failures': [(case.id(), text) for case, text in result.failures],
        'errors': [(case.id(), text) for case, text in result.errors],
        'skipped': [(case.id(), reason) for case, reason in result.skipped],
        'expected_failures': [(case.id(), text) for case, text in result.expectedFailures],
        'unexpected_successes': [case.id() for case in result.unexpectedSuccesses],
    }

if __name__ == '__main__':
    report = run()
    print(json.dumps(report, indent=2))
    sys.exit(0 if len(report['passed']) == len(SELECTED) else 1)
'''


def selected_module(root=ROOT):
    selection = json.loads((root / 'selection.json').read_text())
    upstream = root / 'upstream' / ('cpython-' + selection['cpython_version'])
    for filename, key in [('test_ssl.py', 'source_sha256'), ('LICENSE', 'license_sha256')]:
        if hashlib.sha256((upstream / filename).read_bytes()).hexdigest() != selection[key]:
            raise ValueError('upstream checksum mismatch: ' + filename)
    for filename, expected_hash in selection.get('data_files', {}).items():
        relative = Path(filename)
        if relative.is_absolute() or '..' in relative.parts or not relative.parts or relative.parts[0] != 'certdata':
            raise ValueError('invalid fixture path: ' + filename)
        if hashlib.sha256((upstream / relative).read_bytes()).hexdigest() != expected_hash:
            raise ValueError('upstream fixture checksum mismatch: ' + filename)
    for filename, expected_hash in selection.get('support_files', {}).items():
        relative = Path(filename)
        if relative.is_absolute() or '..' in relative.parts or not relative.parts or relative.parts[0] != 'test':
            raise ValueError('invalid support path: ' + filename)
        if hashlib.sha256((upstream / relative).read_bytes()).hexdigest() != expected_hash:
            raise ValueError('upstream support checksum mismatch: ' + filename)
    tree = ast.parse((upstream / 'test_ssl.py').read_text())
    expected = {f'{name}.{method}' for name, methods in selection['classes'].items() for method in methods}
    if len(expected) != sum(map(len, selection['classes'].values())):
        raise ValueError('duplicate selected test')
    selected, pending, body = set(), [], []
    expected_module_helpers = set(selection.get('module_helpers', []))
    expected_globals = set(selection.get('globals', []))
    found_module_helpers, found_globals = set(), set()
    expected_helpers = {f'{name}.{method}' for name, methods in selection['helpers'].items() for method in methods}
    found_helpers = set()
    for original in tree.body:
        if isinstance(original, ast.FunctionDef) and original.name in expected_module_helpers:
            found_module_helpers.add(original.name)
            body.append(copy.deepcopy(original))
            continue
        if isinstance(original, ast.Assign):
            names = {node.id for node in original.targets if isinstance(node, ast.Name)} & expected_globals
            if names:
                found_globals.update(names)
                body.append(copy.deepcopy(original))
            continue
        if not isinstance(original, ast.ClassDef):
            continue
        methods = [node for node in original.body if isinstance(node, ast.FunctionDef)]
        included = []
        for test in methods:
            name = f'{original.name}.{test.name}'
            if name in expected:
                selected.add(name)
                included.append(test)
            elif name in expected_helpers:
                found_helpers.add(name)
                included.append(test)
            elif test.name.startswith('test_'):
                pending.append(name)
        if included:
            for method in methods:
                if method.name in ('setUp', 'tearDown', 'setUpClass', 'tearDownClass') and method not in included:
                    raise ValueError('unselected lifecycle hook: ' + original.name + '.' + method.name)
            # Keep each method's complete AST, including decorators/assertions.
            subset = copy.deepcopy(original)
            subset.body = copy.deepcopy(included)
            body.append(subset)
    if selected != expected:
        raise ValueError('selected tests missing upstream: ' + ', '.join(sorted(expected - selected)))
    if found_helpers != expected_helpers:
        raise ValueError('selected helpers missing upstream: ' + ', '.join(sorted(expected_helpers - found_helpers)))
    if found_module_helpers != expected_module_helpers:
        raise ValueError('selected module helpers missing upstream: ' + ', '.join(sorted(expected_module_helpers - found_module_helpers)))
    if found_globals != expected_globals:
        raise ValueError('selected globals missing upstream: ' + ', '.join(sorted(expected_globals - found_globals)))
    excluded = selection.get('excluded', {})
    if set(excluded) != set(pending):
        raise ValueError('unclassified upstream tests')
    if any(reason not in ('windows', 'locale', 'timezone', 'external-network', 'python-threads')
           for reason in excluded.values()):
        raise ValueError('unknown exclusion reason')
    if len(selected) + len(excluded) != selection['upstream_test_count']:
        raise ValueError('upstream inventory count changed')
    module = ast.Module(body=body, type_ignores=[])
    metadata = '\n'.join([
        f"CPYTHON_VERSION = {selection['cpython_version']!r}",
        f"SOURCE_SHA256 = {selection['source_sha256']!r}",
        f'SELECTED = {sorted(selected)!r}',
        f'EXCLUDED = {excluded!r}',
        f"FIXTURE_SHA256 = {dict(selection.get('data_files', {}), **selection.get('support_files', {}))!r}",
    ])
    return HEADER + '\n' + metadata + '\n\n' + ast.unparse(module) + '\n' + RUNNER


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    source = selected_module()
    if args.output:
        args.output.write_text(source)
    else:
        # Native reference uses exactly the generated source used by the guest.
        with tempfile.TemporaryDirectory(prefix='ssl-conformance-') as directory:
            path = Path(directory) / 'cpython_ssl_portable.py'
            path.write_text(source)
            upstream_data = ROOT / 'upstream' / ('cpython-' + json.loads((ROOT / 'selection.json').read_text())['cpython_version']) / 'certdata'
            if upstream_data.is_dir():
                shutil.copytree(upstream_data, Path(directory) / 'certdata')
            shutil.copytree(upstream_data.parent / 'test', Path(directory) / 'test')
            return subprocess.call([sys.executable, str(path)],
                                   env=dict(os.environ, TWR_SSL_CONFORMANCE_DATA=directory),
                                   cwd=directory)
    return 0


if __name__ == '__main__':
    sys.exit(main())
