"""Run original portable CPython cases from an installed SDK component."""
import twr_python_sdk
twr_python_sdk.install()

import array
import collections.abc
import contextlib
import dataclasses
import enum
import errno
import functools
import gc
import hashlib
import importlib
import json
import os
import opcode
import pprint
import re
import socket
import ssl
import stat
import string
import subprocess
import sysconfig
import tempfile
import textwrap
import time
import traceback
import types
import unittest
import unittest.mock
import warnings
import weakref

# Include the embedded interpreter's build configuration in the snapshot.
importlib.import_module(sysconfig._get_sysconfigdata_name())


def _ordinary_unittest_probe():
    class FailureProbe(unittest.TestCase):
        def runTest(self):
            with self.assertRaisesRegex(ValueError, 'deliberate regex mismatch'):
                ssl._ASN1Object.fromname('twr-missing-asn1-object')
    result = unittest.TestResult()
    FailureProbe().run(result)
    if result.testsRun != 1 or len(result.failures) != 1 or result.errors:
        raise RuntimeError('ordinary unittest could not record the expected assertion failure')
    return True


twr_python_sdk.freeze_packages("cpython_ssl_portable", "test")


class WitWorld:
    def run(self):
        # Import after instantiation: upstream decorators can query SSL facts.
        os.environ['TWR_SSL_CONFORMANCE_DATA'] = '/ssl-conformance'
        os.chdir('/tmp')
        import cpython_ssl_portable
        report = cpython_ssl_portable.run()
        report['ordinary_unittest_failure_recorded'] = _ordinary_unittest_probe()
        return json.dumps(report)
