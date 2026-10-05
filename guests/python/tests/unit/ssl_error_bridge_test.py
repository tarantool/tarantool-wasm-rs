"""Generated frozen WIT failures must not leak into Python exception chains."""
from dataclasses import dataclass
import enum
import importlib.util
from pathlib import Path
import sys
from types import ModuleType, SimpleNamespace
import unittest
from unittest.mock import patch


@dataclass(frozen=True)
class FrozenWitError(Exception):
    value: object


class Kind(enum.Enum):
    INVALID_INPUT = 0
    WANT_READ = 1
    WANT_WRITE = 2
    ZERO_RETURN = 3
    EOF = 4
    VERIFICATION = 5
    TLS = 6


class ErrorBridgeTests(unittest.TestCase):
    def setUp(self):
        host = ModuleType('wit_world.imports.ssl_bio')
        host.ErrorKind = Kind
        host.describe_error = lambda message: SimpleNamespace(library='SSL', reason='REASON')
        host.verification_message = lambda code: 'certificate rejected'
        world = ModuleType('wit_world')
        world.Err = FrozenWitError
        imports = ModuleType('wit_world.imports')
        imports.ssl_bio = host
        world.imports = imports
        name = '_twr_ssl_error_test'
        path = Path(__file__).resolve().parents[2] / 'ports/ssl/python/tarantool_ssl.py'
        spec = importlib.util.spec_from_file_location(name, path)
        self.ssl = importlib.util.module_from_spec(spec)
        self.modules = patch.dict(sys.modules, {'wit_world': world, 'wit_world.imports': imports,
                                               'wit_world.imports.ssl_bio': host, name: self.ssl})
        self.modules.start()
        spec.loader.exec_module(self.ssl)

    def tearDown(self):
        self.modules.stop()

    def fail(self, kind, code=None):
        raise FrozenWitError(SimpleNamespace(kind=kind, message='host message', verify_code=code))

    def test_success_and_ordinary_python_failure_are_preserved(self):
        token = object()
        self.assertIs(self.ssl._invoke(lambda value: value, token), token)
        failure = RuntimeError('ordinary error')
        def fail():
            raise failure
        with self.assertRaises(RuntimeError) as caught:
            self.ssl._invoke(fail)
        self.assertIs(caught.exception, failure)

    def test_every_host_failure_keeps_its_python_type_and_metadata(self):
        cases = [(Kind.INVALID_INPUT, ValueError, None),
                 (Kind.WANT_READ, self.ssl.SSLWantReadError, self.ssl.SSL_ERROR_WANT_READ),
                 (Kind.WANT_WRITE, self.ssl.SSLWantWriteError, self.ssl.SSL_ERROR_WANT_WRITE),
                 (Kind.ZERO_RETURN, self.ssl.SSLZeroReturnError, self.ssl.SSL_ERROR_ZERO_RETURN),
                 (Kind.EOF, self.ssl.SSLEOFError, self.ssl.SSL_ERROR_EOF),
                 (Kind.VERIFICATION, self.ssl.SSLCertVerificationError, self.ssl.SSL_ERROR_SSL),
                 (Kind.TLS, self.ssl.SSLError, self.ssl.SSL_ERROR_SSL)]
        for kind, expected, number in cases:
            with self.subTest(kind=kind):
                with self.assertRaises(expected) as caught:
                    self.ssl._invoke(self.fail, kind, 62)
                error = caught.exception
                self.assertIs(type(error), expected)
                self.assertEqual(str(error), 'host message' if number is None else '[SSL: REASON] host message')
                self.assertIsNone(error.__context__)
                self.assertIsNone(error.__cause__)
                if number is not None:
                    self.assertEqual(error.errno, number)
                    self.assertEqual((error.library, error.reason), ('SSL', 'REASON'))
                if kind == Kind.VERIFICATION:
                    self.assertEqual((error.verify_code, error.verify_message), (62, 'certificate rejected'))

    def test_standard_unittest_can_record_a_failure_after_a_host_error(self):
        invoke = self.ssl._invoke
        fail = self.fail
        class FailingTest(unittest.TestCase):
            def runTest(self):
                # An intentional failed regex assertion exercises unittest's
                # normal traceback cleanup, including exception contexts.
                with self.assertRaisesRegex(ValueError, 'different message'):
                    invoke(fail, Kind.INVALID_INPUT)
        result = unittest.TestResult()
        FailingTest().run(result)
        self.assertEqual(result.testsRun, 1)
        self.assertEqual(len(result.failures), 1)
        self.assertEqual(result.errors, [])
        self.assertNotIn('FrozenInstanceError', result.failures[0][1])

    def test_missing_cadata_certificates_keep_native_errno_without_changing_trust(self):
        host = self.ssl._host
        @dataclass
        class CertificateInput:
            value: bytes
        host.CertificateInput_Pem = host.CertificateInput_Der = CertificateInput
        message = 'certificate input contains no certificates'
        def prepare_ca(data):
            raise FrozenWitError(SimpleNamespace(kind=Kind.TLS, message=message, verify_code=None))
        host.prepare_ca = prepare_ca
        host.describe_error = lambda text: SimpleNamespace(
            library=None if text == 'certificate input contains no certificates' else 'SSL',
            reason=None if text == 'certificate input contains no certificates' else 'REASON')
        context = self.ssl.SSLContext.__new__(self.ssl.SSLContext)
        context._capaths, context._ca = [], b'existing trust'
        for data, prefix in [('broken', 'no start line'), (b'broken', 'not enough data')]:
            with self.subTest(data=data):
                with self.assertRaises(self.ssl.SSLError) as caught:
                    context.load_verify_locations(cadata=data)
                error = caught.exception
                self.assertEqual(error.errno, 0)
                self.assertEqual(str(error), prefix + ': cadata does not contain a certificate')
                self.assertEqual((error.library, error.reason), (None, None))
                self.assertEqual(context._ca, b'existing trust')
        message = 'another TLS error'
        with self.assertRaises(self.ssl.SSLError) as caught:
            context.load_verify_locations(cadata=b'broken')
        self.assertEqual((caught.exception.errno, caught.exception.library, caught.exception.reason),
                         (self.ssl.SSL_ERROR_SSL, 'SSL', 'REASON'))


if __name__ == '__main__':
    unittest.main()
