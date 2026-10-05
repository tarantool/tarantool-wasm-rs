"""Check WASI DNS adaptation without changing the native test loop policy."""

import asyncio
import importlib.util
from pathlib import Path
import socket
import sys
import unittest
from unittest.mock import patch

_spec = importlib.util.spec_from_file_location(
    'tarantool_asyncio', Path(__file__).resolve().parents[2] /
    'ports/asyncio_dns/python/tarantool_asyncio.py')
adapter = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(adapter)


class ResolverTests(unittest.TestCase):
    def setUp(self):
        self.original = asyncio.BaseEventLoop.getaddrinfo

    def tearDown(self):
        asyncio.BaseEventLoop.getaddrinfo = self.original

    def install(self):
        with patch.object(sys, 'platform', 'wasi'):
            self.assertTrue(adapter.install())

    def test_native_platform_and_custom_loop(self):
        with patch.object(sys, 'platform', 'linux'):
            self.assertFalse(adapter.install())
        self.assertIs(asyncio.BaseEventLoop.getaddrinfo, self.original)
        class CustomLoop(asyncio.SelectorEventLoop):
            async def getaddrinfo(self, *args, **kwargs):
                return ['custom']
        self.install()
        self.install()
        loop = CustomLoop()
        try:
            self.assertEqual(loop.run_until_complete(loop.getaddrinfo('name', 443)), ['custom'])
        finally:
            loop.close()

    def test_arguments_result_and_no_executor(self):
        self.install()
        addresses = [(socket.AF_INET, socket.SOCK_STREAM, 6, '', ('127.0.0.1', 443))]
        async def lookup():
            loop = asyncio.get_running_loop()
            with patch.object(loop, 'run_in_executor', side_effect=AssertionError('executor')):
                return await loop.getaddrinfo(b'name', 'https', family=socket.AF_INET,
                                              type=socket.SOCK_STREAM, proto=6, flags=socket.AI_CANONNAME)
        with patch.object(socket, 'getaddrinfo', return_value=addresses) as resolver:
            self.assertIs(asyncio.run(lookup()), addresses)
            resolver.assert_called_once_with(b'name', 'https', socket.AF_INET,
                                             socket.SOCK_STREAM, 6, socket.AI_CANONNAME)

    def test_debug_and_error_identity(self):
        self.install()
        error = socket.gaierror(socket.EAI_NONAME, 'denied')
        async def lookup():
            loop = asyncio.get_running_loop()
            with patch.object(loop, '_getaddrinfo_debug', side_effect=error) as resolver:
                with self.assertRaises(socket.gaierror) as caught:
                    await loop.getaddrinfo('name', 443)
                self.assertIs(caught.exception, error)
                resolver.assert_called_once_with('name', 443, 0, 0, 0, 0)
        asyncio.run(lookup(), debug=True)

    def test_cancellation_before_lookup(self):
        self.install()
        async def lookup():
            loop = asyncio.get_running_loop()
            task = asyncio.create_task(loop.getaddrinfo('name', 443))
            # Start the adapter and stop at its initial cooperative yield.
            await asyncio.sleep(0)
            task.cancel()
            with self.assertRaises(asyncio.CancelledError):
                await task
        with patch.object(socket, 'getaddrinfo') as resolver:
            asyncio.run(lookup())
            resolver.assert_not_called()

    def test_closed_loop(self):
        self.install()
        loop = asyncio.new_event_loop()
        loop.close()
        with patch.object(socket, 'getaddrinfo') as resolver:
            with self.assertRaisesRegex(RuntimeError, 'Event loop is closed'):
                asyncio.run(loop.getaddrinfo('name', 443))
            resolver.assert_not_called()


if __name__ == '__main__':
    unittest.main()
