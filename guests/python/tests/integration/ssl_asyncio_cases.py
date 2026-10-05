"""Exercise stdlib SSLProtocol, TLS servers and stream upgrades."""
import ssl


def asyncio_ssl_protocol(cert_dir, tls12=False, buffered=False):
    import asyncio
    from asyncio.sslproto import SSLProtocol
    from collections import deque

    class Handle:
        def __init__(self, callback, args):
            self.callback, self.args = callback, args
            self.cancelled = False
        def cancel(self):
            self.cancelled = True

    class Loop:
        def __init__(self):
            self.ready = deque()
            self.errors = []
        def get_debug(self):
            return False
        def call_soon(self, callback, *args, **kwargs):
            handle = Handle(callback, args)
            self.ready.append(handle)
            return handle
        def call_later(self, delay, callback, *args, **kwargs):
            return Handle(callback, args)
        def call_exception_handler(self, details):
            self.errors.append(details)
        def drain(self):
            for _ in range(10000):
                if not self.ready:
                    return
                handle = self.ready.popleft()
                if not handle.cancelled:
                    handle.callback(*handle.args)
            raise AssertionError('SSLProtocol callback queue did not drain')

    class Application(asyncio.BufferedProtocol if buffered else asyncio.Protocol):
        def __init__(self):
            self.transport = None
            self.received = bytearray()
            self.buffer = bytearray(4096)
            self.lost = False
            self.error = None
        def connection_made(self, transport):
            self.transport = transport
        def data_received(self, data):
            self.received.extend(data)
        def get_buffer(self, size):
            return memoryview(self.buffer)
        def buffer_updated(self, size):
            self.received.extend(self.buffer[:size])
        def connection_lost(self, error):
            self.lost, self.error = True, error
        def eof_received(self):
            return None

    class Wire:
        def __init__(self, loop, protocol):
            self.loop, self.protocol = loop, protocol
            self.peer = None
            self.pending = bytearray()
            self.paused = False
            self.closed = False
        def write(self, data):
            assert not self.closed
            self.pending.extend(data)
        def get_write_buffer_size(self):
            return len(self.pending)
        def get_extra_info(self, name, default=None):
            return default
        def is_closing(self):
            return self.closed
        def pause_reading(self):
            self.paused = True
        def resume_reading(self):
            self.paused = False
        def _force_close(self, error):
            if not self.closed:
                self.closed = True
                self.loop.call_soon(self.protocol.connection_lost, error)
        def close(self):
            self._force_close(None)
        def deliver(self, size):
            if not self.pending or self.peer.paused or self.peer.closed:
                return False
            data = bytes(self.pending[:size])
            del self.pending[:len(data)]
            protocol = self.peer.protocol
            offset = 0
            while offset < len(data):
                target = protocol.get_buffer(len(data) - offset)
                count = min(len(target), len(data) - offset)
                target[:count] = data[offset:offset + count]
                protocol.buffer_updated(count)
                offset += count
            return True

    client = ssl.create_default_context(cafile=cert_dir + '/server.crt')
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    version = ssl.TLSVersion.TLSv1_2 if tls12 else ssl.TLSVersion.TLSv1_3
    client.minimum_version = client.maximum_version = version
    server.minimum_version = server.maximum_version = version
    loop = Loop()
    apps = [Application(), Application()]
    protocols = [SSLProtocol(loop, apps[0], client, None, server_hostname='localhost'),
                 SSLProtocol(loop, apps[1], server, None, server_side=True)]
    wires = [Wire(loop, protocol) for protocol in protocols]
    wires[0].peer, wires[1].peer = wires[1], wires[0]

    def pump(done, fragment=32768):
        for _ in range(20000):
            loop.drain()
            assert not loop.errors, loop.errors
            if done():
                return
            progress = any([wire.deliver(fragment) for wire in wires])
            if not progress and not loop.ready:
                raise AssertionError('SSLProtocol stalled')
        raise AssertionError('SSLProtocol iteration limit')

    try:
        for protocol, wire in zip(protocols, wires):
            protocol.connection_made(wire)
        pump(lambda: all(app.transport for app in apps), fragment=17)
        assert all(app.transport.get_extra_info('ssl_object').version() ==
                   ('TLSv1.2' if tls12 else 'TLSv1.3') for app in apps)
        assert apps[0].transport.get_extra_info('peercert')['subject']
        apps[1].transport.pause_reading()
        data = b'asyncio-tls-' * 45000
        apps[0].transport.write(memoryview(data))
        for _ in range(3):
            for wire in wires:
                wire.deliver(32768)
            loop.drain()
        assert not apps[1].received
        apps[1].transport.resume_reading()
        pump(lambda: len(apps[1].received) == len(data))
        assert apps[1].received == data
        apps[1].transport.writelines([b'asyncio-', memoryview(b'reply')])
        pump(lambda: bytes(apps[0].received) == b'asyncio-reply')
        apps[0].transport.close()
        pump(lambda: all(app.lost for app in apps))
        assert all(app.error is None and app.transport.is_closing() for app in apps)
        return 'asyncio-ssl-protocol'
    finally:
        for wire in wires:
            wire._force_close(None)
        loop.drain()


def asyncio_tls_server(cert_dir, tls12=False, upgrade=False):
    import asyncio
    version = ssl.TLSVersion.TLSv1_2 if tls12 else ssl.TLSVersion.TLSv1_3
    server_context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server_context.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    server_context.minimum_version = server_context.maximum_version = version
    client_context = ssl.create_default_context(cafile=cert_dir + '/server.crt')
    client_context.minimum_version = client_context.maximum_version = version
    payload = b'asyncio-server-' * 4096

    async def exchange():
        loop = asyncio.get_running_loop()
        completion = loop.create_future()
        handlers = set()

        async def handle(reader, writer):
            try:
                if upgrade:
                    assert await asyncio.wait_for(reader.readline(), 3) == b'STARTTLS\n'
                    writer.write(b'READY\n')
                    await writer.drain()
                    await writer.start_tls(server_context, ssl_handshake_timeout=3)
                assert writer.get_extra_info('ssl_object').version() == (
                    'TLSv1.2' if tls12 else 'TLSv1.3')
                data = await asyncio.wait_for(reader.readexactly(len(payload)), 5)
                assert data == payload
                writer.write(data)
                await writer.drain()
            except Exception as error:
                if not completion.done():
                    completion.set_exception(error)
            finally:
                writer.close()
                try:
                    await asyncio.wait_for(writer.wait_closed(), 5)
                except Exception as error:
                    if not completion.done():
                        completion.set_exception(error)
                if not completion.done():
                    completion.set_result(None)

        def accepted(reader, writer):
            task = loop.create_task(handle(reader, writer))
            handlers.add(task)
            task.add_done_callback(handlers.discard)

        options = {} if upgrade else {'ssl': server_context, 'ssl_handshake_timeout': 3}
        server = await asyncio.start_server(accepted, '127.0.0.1', 0, **options)
        writer = None
        completed = False
        try:
            port = server.sockets[0].getsockname()[1]
            options = {} if upgrade else {
                'ssl': client_context, 'server_hostname': 'localhost', 'ssl_handshake_timeout': 3}
            reader, writer = await asyncio.open_connection('127.0.0.1', port, **options)
            if upgrade:
                writer.write(b'STARTTLS\n')
                await writer.drain()
                assert await asyncio.wait_for(reader.readline(), 3) == b'READY\n'
                await writer.start_tls(client_context, server_hostname='localhost', ssl_handshake_timeout=3)
            assert writer.get_extra_info('ssl_object').version() == (
                'TLSv1.2' if tls12 else 'TLSv1.3')
            assert writer.get_extra_info('peercert')['subject']
            writer.write(payload)
            await writer.drain()
            assert await asyncio.wait_for(reader.readexactly(len(payload)), 5) == payload
            writer.close()
            await asyncio.wait_for(writer.wait_closed(), 5)
            await asyncio.wait_for(completion, 5)
            completed = True
        finally:
            if writer is not None and not completed:
                writer.transport.abort()
            server.close()
            await server.wait_closed()
            pending = list(handlers)
            for task in pending:
                task.cancel()
            await asyncio.gather(*pending, return_exceptions=True)
            if completion.done() and not completion.cancelled():
                completion.exception()
    asyncio.run(exchange())
    return 'asyncio-tls-server'


if __name__ == '__main__':
    import sys
    for tls12 in (False, True):
        for buffered in (False, True):
            print(tls12, buffered, asyncio_ssl_protocol(sys.argv[1], tls12, buffered))

    for tls12 in (False, True):
        for upgrade in (False, True):
            print('server', tls12, upgrade, asyncio_tls_server(sys.argv[1], tls12, upgrade))
