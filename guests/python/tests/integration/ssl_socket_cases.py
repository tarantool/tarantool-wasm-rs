"""Shared socket checks for native Python 3.12 and the Wasm SSL adapter."""
import http.client
import errno
import socket
import ssl
import time
import urllib.request


def context(cert_dir):
    return ssl.create_default_context(cafile=cert_dir + "/server.crt")


def connect(cert_dir, port, **kwargs):
    raw = socket.create_connection(("127.0.0.1", port), timeout=3)
    try:
        wrapped = context(cert_dir).wrap_socket(raw, server_hostname="localhost", **kwargs)
        assert raw.fileno() == -1
        return wrapped
    except BaseException:
        raw.close()
        raise


def private_socket_arguments():
    import _ssl
    import gc
    import weakref
    context = _ssl._SSLContext(_ssl.PROTOCOL_TLS_CLIENT)
    with socket.socket() as raw:
        fd = raw.fileno()
        # Both closures run before raw is deleted below.
        for call in (lambda: context._wrap_socket(object(), False),
                     lambda: context._wrap_socket(raw, False, owner=1)):  # noqa: F821
            try:
                call()
            except TypeError:
                pass
            else:
                raise AssertionError('invalid private raw-socket argument accepted')
            assert raw.fileno() == fd
        peer = context._wrap_socket(raw, False)
        assert peer.owner is None and peer.context is context
        assert peer.server_side is False and peer.server_hostname is None
        assert peer.version() is None and peer.read(0) == b''
        del peer
        gc.collect()
        assert raw.fileno() == fd, 'private peer took ownership of TCP'
    raw = socket.socket()
    reference = weakref.ref(raw)
    peer = context._wrap_socket(raw, False)
    del raw
    gc.collect()
    assert reference() is None, 'private TLS wrapper retains the raw socket'
    for call in (peer.do_handshake, lambda: peer.write(b'x'),
                 lambda: peer.read(1), peer.shutdown):
        try:
            call()
        except ssl.SSLError as error:
            assert error.errno == 0, error
            assert 'Underlying socket connection gone' in str(error)
        else:
            raise AssertionError('collected raw socket was usable')
    return 'private-socket-arguments'


def private_socket_exchange(cert_dir, port, tls12=False):
    import _ssl
    context = _ssl._SSLContext(_ssl.PROTOCOL_TLS_CLIENT)
    context.load_verify_locations(cafile=cert_dir + '/server.crt')
    if tls12:
        context.maximum_version = ssl.TLSVersion.TLSv1_2
    seen = []
    class Owner:
        pass
    owner = Owner()
    def message(peer, direction, version, content, kind, data):
        assert peer is owner and type(version) is type(content) is type(kind) is int
        seen.append(direction)
    context._msg_callback = message
    with socket.create_connection(('127.0.0.1', port), timeout=3) as raw:
        fd = raw.fileno()
        peer = context._wrap_socket(raw, False, 'localhost', owner=owner)
        assert raw.fileno() == fd and peer.owner is owner
        peer.do_handshake()
        assert raw.gettimeout() == 3 and raw.fileno() == fd
        assert peer.version() == ('TLSv1.2' if tls12 else 'TLSv1.3')
        assert peer.getpeercert()['subject'] and peer.cipher() and peer.session
        assert peer.get_verified_chain()[0].public_bytes(2) == peer.getpeercert(True)
        assert peer.write(memoryview(b'Eprivate-socket')) == 15
        buffer = bytearray(14)
        assert peer.read(14, buffer) == 14 and buffer == b'private-socket'
        assert seen and 'read' in seen and 'write' in seen
    # Verification errors release no descriptor owned by the caller.
    with socket.create_connection(('127.0.0.1', port), timeout=3) as raw:
        fd = raw.fileno()
        peer = context._wrap_socket(raw, False, 'wrong.example', owner=owner)
        try:
            peer.do_handshake()
        except ssl.SSLCertVerificationError as error:
            assert error.verify_code == 62
        else:
            raise AssertionError('private raw socket ignored hostname verification')
        assert raw.fileno() == fd and raw.gettimeout() == 3
    return 'private-socket-exchange'


def private_socket_nonblocking(cert_dir, port):
    import _ssl
    import select
    context = _ssl._SSLContext(_ssl.PROTOCOL_TLS_CLIENT)
    context.load_verify_locations(cafile=cert_dir + '/server.crt')
    messages = []
    context._msg_callback = lambda peer, *args: messages.append(peer)
    with socket.create_connection(('127.0.0.1', port), timeout=3) as raw:
        raw.setblocking(False)
        peer = context._wrap_socket(raw, False, 'localhost')
        deadline = time.monotonic() + 3
        while True:
            try:
                peer.do_handshake()
                break
            except ssl.SSLWantReadError:
                assert select.select([raw], [], [], max(0, deadline - time.monotonic()))[0]
            except ssl.SSLWantWriteError:
                assert select.select([], [raw], [], max(0, deadline - time.monotonic()))[1]
        assert raw.gettimeout() == 0 and peer.read(0) == b''
        assert messages and all(value is raw for value in messages)
        try:
            peer.read(1)
        except ssl.SSLWantReadError:
            pass
        else:
            raise AssertionError('nonblocking private read did not require readiness')
        raw.settimeout(0.03)
        try:
            peer.read(1)
        except TimeoutError:
            pass
        else:
            raise AssertionError('private read ignored timeout')
        assert raw.gettimeout() == 0.03
        raw.settimeout(3)
        assert peer.write(b'Eafter-timeout') == 14
        assert peer.read(13) == b'after-timeout'
    return 'private-socket-nonblocking'


def private_socket_closed_transport(cert_dir, port):
    import _ssl
    context = _ssl._SSLContext(_ssl.PROTOCOL_TLS_CLIENT)
    context.load_verify_locations(cafile=cert_dir + '/server.crt')
    context._msg_callback = lambda raw, *args: raw.close()
    with socket.create_connection(('127.0.0.1', port), timeout=3) as raw:
        peer = context._wrap_socket(raw, False, 'localhost')
        try:
            peer.do_handshake()
        except OSError as error:
            assert error.errno == errno.EBADF, error
        else:
            raise AssertionError('callback-closed raw socket was usable')
        assert raw.fileno() == -1
        if ssl.__name__ == 'tarantool_ssl':
            assert peer._engine._message_driver_active is False
            assert peer._engine._defer_message_errors is False
            assert peer._engine._in_message_callback is False
    return 'private-socket-closed-transport'


def private_socket_tracebacks(cert_dir, tls_port, plain_port):
    import _ssl
    import gc
    import weakref
    context = _ssl._SSLContext(_ssl.PROTOCOL_TLS_CLIENT)
    context.load_verify_locations(cafile=cert_dir + '/server.crt')
    retained = []
    for method in ('constructor', 'owner-constructor', 'owner-bio-constructor',
                   'invalid-owner-constructor', 'owner-setter', 'setup',
                   'session-constructor', 'hostname-constructor', 'side-constructor',
                   'session-bio-constructor', 'hostname-bio-constructor',
                   'incoming-bio-constructor', 'outgoing-bio-constructor', 'side-bio-constructor',
                   'do_handshake', 'write', 'read', 'shutdown'):
        raw = socket.create_connection(('127.0.0.1', plain_port if method in ('do_handshake', 'write') else tls_port), timeout=3)
        reference = weakref.ref(raw)
        peer = context._wrap_socket(raw, False, 'localhost')
        if method in ('read', 'shutdown'):
            peer.do_handshake()
        if method == 'read':
            raw.settimeout(0.03)
        else:
            raw.setblocking(False)
        try:
            if method == 'constructor':
                context._wrap_socket(raw, False, '')
            elif method == 'owner-constructor':
                context._wrap_socket(raw, False, '', owner=raw)
            elif method == 'owner-bio-constructor':
                context._wrap_bio(_ssl.MemoryBIO(), _ssl.MemoryBIO(), False, '', owner=raw)
            elif method == 'invalid-owner-constructor':
                context._wrap_socket(raw, False, 'localhost', owner=[raw])
            elif method == 'owner-setter':
                peer.owner = [raw]
            elif method == 'session-constructor':
                context._wrap_socket(raw, False, 'localhost', session=raw)
            elif method == 'hostname-constructor':
                context._wrap_socket(raw, False, raw)
            elif method == 'side-constructor':
                context._wrap_socket(raw, raw, 'localhost')
            elif method == 'session-bio-constructor':
                context._wrap_bio(_ssl.MemoryBIO(), _ssl.MemoryBIO(), False, 'localhost', session=raw)
            elif method == 'hostname-bio-constructor':
                context._wrap_bio(_ssl.MemoryBIO(), _ssl.MemoryBIO(), False, raw)
            elif method == 'incoming-bio-constructor':
                context._wrap_bio(raw, _ssl.MemoryBIO(), False, 'localhost')
            elif method == 'outgoing-bio-constructor':
                context._wrap_bio(_ssl.MemoryBIO(), raw, False, 'localhost')
            elif method == 'side-bio-constructor':
                context._wrap_bio(_ssl.MemoryBIO(), _ssl.MemoryBIO(), raw, 'localhost')
            elif method == 'setup':
                raw.close()
                peer.do_handshake()
            elif method in ('read', 'write'):
                getattr(peer, method)(b'x' if method == 'write' else 1)
            else:
                getattr(peer, method)()
        except (OSError, ValueError, TypeError) as error:
            retained.append(error)
        else:
            raise AssertionError('expected a transport/readiness failure')
        # ResourceWarning formats socket repr before filtering warnings. The
        # pinned SDK must map getpeername's InvalidState to ENOTCONN, not abort.
        assert repr(raw).startswith('<socket.socket')
        if raw.fileno() >= 0:
            try:
                raw.getpeername()
            except OSError:
                # Kernels can report different errors for a peer closed while
                # connect/handshake was pending; each must remain an OSError.
                pass
        del raw
        gc.collect()
        assert reference() is None, (method, 'retained exception owns raw socket')
    assert all(error.__traceback__ is not None for error in retained)
    return 'private-socket-tracebacks'


def private_socket_listener(cert_dir, port, public=False):
    import _ssl
    context = (ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER) if public
               else _ssl._SSLContext(_ssl.PROTOCOL_TLS_SERVER))
    context.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    seen = []
    def sni(owner, name, context):
        seen.append((owner, name, context))
        if ssl.__name__ == 'tarantool_ssl':
            import io
            engine = peer._sslobj if public else peer._engine
            file = io.BytesIO(b'callback-file')
            file.mode = 'rb'
            file.seek(2)
            def snapshot():
                return (owner.gettimeout(), owner.fileno(), getattr(peer, '_closed', False),
                        bytes(engine._outgoing._buffer), engine._message_driver_active,
                        engine._defer_message_errors, file.tell())
            before = snapshot()
            assert before[4:6] == (True, True)
            calls = (peer.do_handshake, lambda: peer.read(1), lambda: peer.write(b'x'),
                     peer.unwrap if public else peer.shutdown, engine.close)
            if public:
                calls += (peer.close, peer._real_close, lambda: peer.sendall(b''),
                          lambda: peer.shutdown(socket.SHUT_RDWR),
                          lambda: peer.do_handshake(block=True), lambda: peer.sendfile(file))
            for call in calls:
                try:
                    call()
                except ssl.SSLError:
                    pass
                else:
                    raise AssertionError('nested SNI TLS I/O was accepted')
                after = snapshot()
                assert after == before, (before, after)
    context.sni_callback = sni
    with socket.socket() as listener:
        listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        listener.settimeout(3)
        listener.bind(('127.0.0.1', port))
        listener.listen(1)
        raw, address = listener.accept()
        with raw:
            raw.settimeout(3)
            fd = raw.fileno()
            peer = (context.wrap_socket(raw, server_side=True, do_handshake_on_connect=False) if public
                    else context._wrap_socket(raw, True))
            plain = None
            try:
                peer.do_handshake()
                assert seen == [(peer if public else raw, 'localhost', context)], seen
                assert peer.server_side is True
                if not public:
                    assert peer.owner is None
                data = b''
                while len(data) < len(b'listener-client'):
                    block = peer.read(3)
                    assert block
                    data += block
                assert data == b'listener-client'
                data = memoryview(b'listener-server')
                while data:
                    count = peer.write(data[:3])
                    assert count > 0
                    data = data[count:]
                plain = peer.unwrap() if public else peer.shutdown()
                if not public:
                    assert plain is raw
                assert plain.fileno() == fd and plain.gettimeout() == 3
            finally:
                if public:
                    peer.close()
                    if plain is not None:
                        plain.close()
    return 'private-socket-listener'


def private_socket_shutdown(cert_dir, port):
    import _ssl
    context = _ssl._SSLContext(_ssl.PROTOCOL_TLS_CLIENT)
    context.load_verify_locations(cafile=cert_dir + '/server.crt')
    with socket.create_connection(('127.0.0.1', port), timeout=3) as raw:
        fd = raw.fileno()
        peer = context._wrap_socket(raw, False, 'localhost')
        peer.do_handshake()
        peer.write(b'U')
        assert peer.shutdown() is raw
        assert raw.fileno() == fd and raw.gettimeout() == 3
        assert raw.recv(64) == b'plain-after-tls'
        del peer
        assert raw.fileno() == fd
    return 'private-socket-shutdown'


def socket_empty_write_handshake(cert_dir, port):
    with connect(cert_dir, port, do_handshake_on_connect=False) as peer:
        assert peer.version() is None
        assert peer.write(b'') == 0
        assert peer.version() in ('TLSv1.2', 'TLSv1.3')
        assert peer.send(b'') == 0
        peer.sendall(b'Eafter-empty')
        assert peer.read(11) == b'after-empty'
    return 'socket-empty-write-handshake'


def socket_buffer_rules(cert_dir, port):
    from array import array
    with connect(cert_dir, port) as peer:
        peer.sendall(b'E')
        for data in (memoryview(b'abcd')[::2], memoryview(bytearray(b'abcdef'))[::-2]):
            try:
                peer.write(data)
            except BufferError:
                pass
            else:
                raise AssertionError('socket flattened a non-contiguous TLS write')
        for size in (0, -1, -5, 100):
            peer.sendall(b'abcd')
            target = bytearray(b'xxxxxxxx')
            assert peer.read(size, target) == 4
            assert target == b'abcdxxxx'
        for target in (array('I', [0]), memoryview(bytearray(4)).cast('B', shape=[2, 2])):
            peer.sendall(b'abcd')
            assert peer.recv_into(target) == 4
            assert bytes(memoryview(target).cast('B')) == b'abcd'
        peer.setblocking(False)
        assert peer.recv(0) == peer.read(0) == b''
        assert peer.recv_into(bytearray()) == 0
        peer.settimeout(3)
        peer.sendall(b'after-errors')
        assert peer.read(12) == b'after-errors'
    for call in (lambda: peer.read(1), lambda: peer.read(0), lambda: peer.write(b'x')):
        try:
            call()
        except ValueError:
            pass
        else:
            raise AssertionError('TLS I/O on a closed socket was accepted')
    with connect(cert_dir, port) as peer:
        peer.sendall(b'U')
        peer.unwrap()
        for call in (lambda: peer.read(1), lambda: peer.write(b'x')):
            try:
                call()
            except ValueError:
                pass
            else:
                raise AssertionError('TLS I/O on an unwrapped socket was accepted')
        assert peer.recv(64) == b'plain-after-tls'
        assert peer.sendall(b'') is None
        if hasattr(socket, 'MSG_DONTWAIT'):
            assert peer.sendall(b'', socket.MSG_DONTWAIT) is None
    return 'socket-buffer-rules'


def socket_unconnected_buffer_rules(cert_dir, port):
    with context(cert_dir).wrap_socket(socket.socket(), server_hostname='localhost',
                                      do_handshake_on_connect=False) as peer:
        assert peer._sslobj is None and peer.version() is None
        for call in (lambda: peer.read(0),
                     lambda: peer.read(0, bytearray()),
                     lambda: peer.recv_into(bytearray()), lambda: peer.write(b'')):
            try:
                call()
            except ValueError:
                pass
            else:
                raise AssertionError('unconnected socket exposed a TLS wrapper')
        assert peer.recv(0) == b''
        assert peer.recv_into(bytearray(), 0) == 0
        for flags in (0, 1):
            try:
                peer.sendall(b'', flags)
            except OSError:
                pass
            else:
                raise AssertionError('unconnected sendall skipped the raw socket')
        # connect_ex creates the native wrapper before TCP completes. Its zero
        # reads are valid even when the handshake has not started yet.
        peer.setblocking(False)
        result = peer.connect_ex(('127.0.0.1', port))
        assert result in (0, errno.EINPROGRESS, errno.EWOULDBLOCK, errno.EALREADY)
        assert peer.read(0) == peer.recv(0) == b''
        assert peer.read(0, bytearray()) == peer.recv_into(bytearray()) == 0
        assert peer.sendall(b'') is None and peer.version() is None
        try:
            peer.sendall(b'', 1)
        except ValueError:
            pass
        else:
            raise AssertionError('begun TLS connection accepted sendall flags')
        import select
        assert select.select([], [peer], [], 3)[1]
        peer.settimeout(3)
        peer.do_handshake()
        peer.write(b'Eafter-zero')
        assert peer.read(64) == b'after-zero'
    return 'socket-unconnected-buffer-rules'


def socket_pending_buffer_rules(cert_dir, port):
    # The adapter owns its ciphertext queue; native Python has no equivalent
    # guest-visible queue to fault-inject. Shared normal cases are above.
    with connect(cert_dir, port) as peer:
        if not hasattr(peer, '_outgoing'):
            return 'socket-pending-buffer-rules'
        original_send = socket.socket.send
        def blocked(sock, *args, **kwargs):
            if sock is peer:
                raise BlockingIOError(errno.EWOULDBLOCK, 'fixture full send buffer')
            return original_send(sock, *args, **kwargs)
        try:
            peer.setblocking(False)
            socket.socket.send = blocked
            assert peer.write(b'Equeued') == 7
        finally:
            socket.socket.send = original_send
        wire = bytes(peer._outgoing._buffer)
        assert wire
        initial = (peer.gettimeout(), peer._sslobj._defer_message_errors,
                   peer._sslobj._message_driver_active)
        original_flush = peer._flush_output
        original_operation = peer._sslobj._operation
        attempted = []
        def reset_transport(*args, **kwargs):
            attempted.append('flush')
            raise ConnectionResetError('fixture peer reset')
        def entered_tls(*args, **kwargs):
            attempted.append('TLS')
            raise AssertionError('zero/rejected read entered OpenSSL or callbacks')
        class Size:
            calls = 0
            def __index__(self):
                self.calls += 1
                return 0
        try:
            peer._flush_output = reset_transport
            peer._sslobj._operation = entered_tls
            size = Size()
            assert peer.read(size) == b'' and size.calls == 1
            assert peer.recv(0) == b''
            assert peer.read(0, bytearray()) == peer.recv_into(bytearray()) == 0
            for call, kind in (
                (lambda: peer.write(memoryview(b'abcdef')[::2]), BufferError),
                (lambda: peer.send(memoryview(b'abcdef')[::2]), BufferError),
                (lambda: peer.write('text'), TypeError),
                (lambda: peer.read(-1), ValueError),
                (lambda: peer.read(0, b'abcd'), TypeError),
                (lambda: peer.read(1, memoryview(bytearray(8))[::2]), TypeError),
                (lambda: peer.recv_into(memoryview(bytearray(8))[::2]), TypeError)):
                try:
                    call()
                except kind:
                    pass
                else:
                    raise AssertionError('invalid pending-socket operation accepted')
            assert not attempted
            assert bytes(peer._outgoing._buffer) == wire
            assert (peer.gettimeout(), peer._sslobj._defer_message_errors,
                    peer._sslobj._message_driver_active) == initial
        finally:
            peer._flush_output = original_flush
            peer._sslobj._operation = original_operation
        peer.settimeout(3)
        peer.write(b'after')
        response = b''
        while len(response) < 11:
            response += peer.read(11 - len(response))
        assert response == b'queuedafter'
    return 'socket-pending-buffer-rules'


def https_clients(cert_dir, port):
    ctx = context(cert_dir)
    ctx.set_alpn_protocols(["http/1.1"])
    connection = http.client.HTTPSConnection("localhost", port, context=ctx, timeout=3)
    connection.request("GET", "/")
    assert connection.sock.selected_alpn_protocol() == "http/1.1"
    response = connection.getresponse()
    # HTTPConnection may close its socket before the buffered file is consumed.
    assert response.status == 200 and response.read() == b"wasi-https-ok"
    response.close()
    connection.close()
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}),
                                        urllib.request.HTTPSHandler(context=ctx))
    with opener.open("https://localhost:" + str(port) + "/", timeout=3) as response:
        assert response.read() == b"wasi-https-ok"
    return "http.client-and-urllib"


def urllib3_clients(cert_dir, port):
    import urllib3
    assert urllib3.__version__ == "2.8.0"
    assert ssl.OPENSSL_VERSION.startswith("OpenSSL ")
    assert ssl.OPENSSL_VERSION_INFO >= (1, 1, 1)
    number = ssl.OPENSSL_VERSION_NUMBER
    assert ssl.OPENSSL_VERSION_INFO == ((number >> 28) & 15, (number >> 20) & 255,
                                        (number >> 12) & 255, (number >> 4) & 255, number & 15)
    manager = urllib3.PoolManager(ca_certs=cert_dir + "/ca.crt", retries=False,
                                  timeout=urllib3.Timeout(connect=3, read=3))
    try:
        response = manager.request("GET", "https://localhost:" + str(port) + "/", preload_content=False)
        try:
            assert response.status == 200
            assert b"".join(response.stream(3)) == b"wasi-https-ok"
        finally:
            response.close()
        response = manager.request("GET", "https://localhost:" + str(port) + "/gzip")
        assert response.headers["content-encoding"] == "gzip" and response.data == b"wasi-https-ok"
        response.close()
        data = b"urllib3-stream-body" * 100000
        response = manager.request("POST", "https://localhost:" + str(port) + "/", body=data)
        assert response.status == 200 and response.data == data
        response.close()
    finally:
        manager.clear()
    return "urllib3-https"


def urllib3_certificate_rejections(cert_dir, port):
    import urllib3
    for kwargs in ({"ca_certs": cert_dir + "/decoded.crt"},
                   {"ca_certs": cert_dir + "/ca.crt", "assert_hostname": "wrong.example"}):
        manager = urllib3.PoolManager(retries=False, timeout=3, **kwargs)
        try:
            try:
                manager.request("GET", "https://localhost:" + str(port) + "/")
            except urllib3.exceptions.SSLError:
                pass
            else:
                raise AssertionError("urllib3 skipped certificate verification")
        finally:
            manager.clear()
    return "urllib3-certificate-rejections"


def urllib3_denied_connect(cert_dir, port):
    import urllib3
    manager = urllib3.PoolManager(ca_certs=cert_dir + "/ca.crt", retries=False, timeout=0.5)
    try:
        try:
            manager.request("GET", "https://localhost:" + str(port) + "/")
        except urllib3.exceptions.NewConnectionError as error:
            assert isinstance(error.__cause__, PermissionError)
            return "urllib3-network-denied"
        raise AssertionError("urllib3 bypassed WASI network capability")
    finally:
        manager.clear()


def requests_resources():
    import certifi
    import charset_normalizer
    import idna
    import importlib.resources
    import os
    import requests
    import sys
    assert requests.__version__ == "2.34.2"
    assert idna.encode("пример.рф").decode() == "xn--e1afmkfd.xn--p1ai"
    assert charset_normalizer.from_bytes(b"plain ascii").best().encoding == "ascii"
    ca_path = certifi.where()
    if sys.platform == "wasi":
        assert ca_path.startswith("/tmp/")
    assert os.path.isfile(ca_path)
    with open(ca_path, "rb") as file:
        data = file.read()
    assert data == importlib.resources.files("certifi").joinpath("cacert.pem").read_bytes()
    assert data.decode("ascii") == certifi.contents()
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    context.load_verify_locations(cafile=ca_path)
    assert context.cert_store_stats()["x509_ca"] > 100
    return "requests-resources"


def requests_clients(cert_dir, port):
    import requests
    with requests.Session() as session:
        session.trust_env = False
        session.verify = cert_dir + "/ca.crt"
        url = "https://localhost:" + str(port)
        with session.get(url + "/", stream=True, timeout=3) as response:
            assert response.status_code == 200
            assert b"".join(response.iter_content(3)) == b"wasi-https-ok"
        with session.get(url + "/gzip", timeout=3) as response:
            assert response.text == "wasi-https-ok"
        for charset, text in (("cp1252", "caf\u00e9 \u20ac"), ("shift_jis", "\u65e5\u672c\u8a9e"),
                              ("utf-8-sig", "BOM \u043f\u0440\u0438\u0432\u0435\u0442")):
            with session.get(url + "/" + charset, timeout=3) as response:
                assert response.encoding == charset and response.text == text
        data = b"requests-body" * 100000
        with session.post(url + "/", data=data, timeout=3) as response:
            assert response.content == data
    return "requests-https"


def requests_default_verification(port):
    import requests
    with requests.Session() as session:
        session.trust_env = False
        try:
            session.get("https://localhost:" + str(port) + "/", timeout=3)
        except requests.exceptions.SSLError:
            return "requests-default-verification"
        raise AssertionError("default certifi trust accepted the private test CA")


def requests_missing_temp():
    try:
        import requests
    except FileNotFoundError as error:
        assert "No usable temporary directory" in str(error)
        return "requests-needs-guest-temp"
    raise AssertionError("resource extraction bypassed guest filesystem capability")


def requests_denied_connect(cert_dir, port):
    import requests
    import urllib3
    with requests.Session() as session:
        session.trust_env = False
        session.verify = cert_dir + "/ca.crt"
        try:
            session.get("https://localhost:" + str(port) + "/", timeout=0.5)
        except requests.exceptions.ConnectionError as error:
            retry_error = error.args[0]
            assert isinstance(retry_error, urllib3.exceptions.MaxRetryError)
            assert isinstance(retry_error.reason, urllib3.exceptions.NewConnectionError)
            assert isinstance(retry_error.reason.__cause__, PermissionError)
            return "requests-network-denied"
        raise AssertionError("Requests bypassed WASI network capability")


def listener_live_sni(cert_dir, port, mode):
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    seen = []
    def old(peer, name, origin):
        raise AssertionError('replaced or disabled socket callback invoked')
    def selected(peer, name, origin):
        assert isinstance(peer, ssl.SSLSocket) and origin is context and name == 'localhost'
        seen.append(name)
    if mode != 'enable':
        context.sni_callback = old
    with socket.socket() as raw:
        raw.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        raw.bind(('127.0.0.1', port))
        raw.listen(4)
        raw.settimeout(3)
        with context.wrap_socket(raw, server_side=True, do_handshake_on_connect=False) as listener:
            peer, address = listener.accept()
            with peer:
                assert peer.version() is None and address[0] == '127.0.0.1'
                context.sni_callback = None if mode == 'disable' else selected
                peer.do_handshake()
                assert seen == ([] if mode == 'disable' else ['localhost'])
                assert context.session_stats()['accept_good'] == 1
                request = b''
                while len(request) < len(b'listener-client'):
                    chunk = peer.recv(3)
                    assert chunk
                    request += chunk
                assert request == b'listener-client'
                peer.sendall(b'listener-server')
                peer.unwrap().close()
    return 'listener-exchange'


def listener_exchange(cert_dir, port, deferred=False, reject_first=False, sni=False, post_auth=False):
    if isinstance(sni, str) and sni.startswith('live-'):
        return listener_live_sni(cert_dir, port, sni[5:])
    server_context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server_context.load_cert_chain(cert_dir + "/server.crt", cert_dir + "/server.key")
    if post_auth:
        server_context.post_handshake_auth = True
        server_context.verify_mode = ssl.CERT_REQUIRED
        server_context.load_verify_locations(cafile=cert_dir + '/server.crt')
    names = []
    alternate = server_context
    if sni:
        alternate = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        alternate.load_cert_chain(cert_dir + "/server.crt", cert_dir + "/server.key")
        def choose(peer, name, context):
            assert isinstance(peer, ssl.SSLSocket) and context is server_context
            peer.context = alternate
            names.append(name)
            if reject_first and len(names) == 1:
                return ssl.ALERT_DESCRIPTION_UNRECOGNIZED_NAME
        server_context.sni_callback = choose
        if sni in ('pre-both', 'pre-enable', 'pre-disable'):
            assert deferred and not reject_first
            def unexpected(*args):
                raise AssertionError('old socket context callback invoked')
            server_context.sni_callback = None if sni == 'pre-enable' else unexpected
            if sni == 'pre-disable':
                def fallback(peer, name, context):
                    assert isinstance(peer, ssl.SSLSocket)
                    assert context is server_context and peer.context is alternate
                    names.append(name)
                server_context.sni_callback = fallback
            if sni != 'pre-disable':
                def selected(peer, name, context):
                    assert isinstance(peer, ssl.SSLSocket)
                    assert context is alternate and peer.context is alternate
                    names.append(name)
                alternate.sni_callback = selected
    with socket.socket() as raw:
        raw.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        raw.bind(("127.0.0.1", port))
        raw.listen(4)
        raw.settimeout(3)
        with server_context.wrap_socket(raw, server_side=True,
                                        do_handshake_on_connect=not deferred) as listener:
            if reject_first:
                try:
                    peer, address = listener.accept()
                except ssl.SSLError:
                    assert listener.fileno() >= 0
                else:
                    peer.close()
                    raise AssertionError("listener accepted a plaintext TLS peer")
            peer, address = listener.accept()
            with peer:
                assert isinstance(peer, ssl.SSLSocket) and peer.server_side
                assert peer.context is (server_context if deferred else alternate) and address[0] == "127.0.0.1"
                if deferred:
                    assert peer.version() is None
                else:
                    assert peer.version() in ("TLSv1.2", "TLSv1.3")
                peer.settimeout(3)
                if deferred:
                    if isinstance(sni, str):
                        peer.context = alternate
                    peer.do_handshake()
                assert peer.context is alternate
                if sni:
                    assert names == ["localhost"] * (2 if reject_first else 1)
                assert peer.get_channel_binding()
                assert peer.cipher() in peer.shared_ciphers()
                if post_auth:
                    assert peer.version() == 'TLSv1.3' and peer.getpeercert() is None
                    peer.verify_client_post_handshake()
                    peer.sendall(b'auth-request')
                request = b""
                while len(request) < len(b"listener-client"):
                    chunk = peer.recv(3)
                    assert chunk
                    request += chunk
                assert request == b"listener-client"
                if post_auth:
                    assert peer.getpeercert()['subject']
                peer.sendall(b"listener-server")
                peer.unwrap().close()
    return "listener-exchange"


def nonblocking_listener(cert_dir, port):
    server_context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server_context.load_cert_chain(cert_dir + "/server.crt", cert_dir + "/server.key")
    with socket.socket() as raw:
        raw.bind(("127.0.0.1", port))
        raw.listen()
        raw.setblocking(False)
        with server_context.wrap_socket(raw, server_side=True) as listener:
            assert listener.gettimeout() == 0
            try:
                listener.accept()
            except BlockingIOError:
                pass
            else:
                raise AssertionError("empty nonblocking listener accepted a peer")
    return "nonblocking-listener"


def denied_listener(port):
    server_context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    with socket.socket() as raw:
        with server_context.wrap_socket(raw, server_side=True) as listener:
            try:
                listener.bind(("127.0.0.1", port))
            except PermissionError:
                return "listener-network-denied"
            raise AssertionError("TLS listener bypassed WASI network capability")


def large_post(cert_dir, port):
    data = b"tls-socket-contents" * 120000
    connection = http.client.HTTPSConnection("127.0.0.1", port, context=context(cert_dir), timeout=5)
    try:
        connection.request("POST", "/", body=data)
        response = connection.getresponse()
        assert response.read() == data
        response.close()
    finally:
        connection.close()
    return len(data)


def deferred_handshake_and_unwrap(cert_dir, port):
    raw = socket.socket()
    raw.settimeout(3)
    peer = context(cert_dir).wrap_socket(raw, server_hostname="localhost",
                                        do_handshake_on_connect=False)
    try:
        peer.connect(("127.0.0.1", port))
        peer.do_handshake()
        assert peer.version() in ("TLSv1.2", "TLSv1.3")
        assert peer.getpeercert(binary_form=True)
        peer.sendall(b"U")
        plain = peer.unwrap()
        assert plain.recv(100) == b"plain-after-tls"
    finally:
        peer.close()
    return "deferred-handshake-and-unwrap"


def custom_socket_classes(cert_dir, port):
    created = []
    allocations = []
    class Custom(ssl.SSLSocket):
        def __new__(cls, *, family, type, proto, fileno):
            allocations.append((family, type, proto, fileno))
            return super().__new__(cls, family=family, type=type, proto=proto, fileno=fileno)
        def __init__(self, *args, **kwargs):
            raise AssertionError('public socket constructor used')
        @classmethod
        def _create(cls, **kwargs):
            created.append(kwargs['server_hostname'])
            return super()._create(**kwargs)
        def write(self, data):
            self.writes = getattr(self, 'writes', 0) + 1
            return super().write(data)
    class UnusedBIO(ssl.SSLObject):
        @classmethod
        def _create(cls, *args, **kwargs):
            raise AssertionError('BIO factory used for socket')
    ctx = context(cert_dir)
    ctx.sslobject_class = UnusedBIO
    ctx.sslsocket_class = Custom
    for unconnected in (False, True):
        raw = socket.socket() if unconnected else socket.create_connection(('127.0.0.1', port), timeout=3)
        raw.settimeout(3)
        expected_allocation = (raw.family, raw.type, raw.proto, raw.fileno())
        peer = ctx.wrap_socket(raw, server_hostname='localhost')
        assert allocations[-1] == expected_allocation
        assert raw.fileno() == -1 and type(peer) is Custom
        with peer:
            if unconnected:
                peer.connect(('127.0.0.1', port))
            assert peer.server_hostname == 'localhost'
            peer.write(b'E')
            peer.write(b'custom socket')
            response = b''
            while len(response) < len(b'custom socket'):
                response += peer.read(100)
            assert response == b'custom socket' and peer.writes == 2
    assert created == ['localhost', 'localhost']
    deferred_fields = ('_sslobj', '_incoming', '_outgoing', '_record_header',
                       '_record_remaining', '_reading_header', '_session_pending')
    class Slotted(ssl.SSLSocket):
        __slots__ = deferred_fields
    ctx.sslsocket_class = Slotted
    for unconnected in (False, True):
        raw = socket.socket() if unconnected else socket.create_connection(('127.0.0.1', port), timeout=3)
        raw.settimeout(3)
        with ctx.wrap_socket(raw, server_hostname='localhost') as peer:
            assert type(peer) is Slotted and raw.fileno() == -1
            if unconnected:
                peer.connect(('127.0.0.1', port))
            peer.write(b'Eslotted socket')
            assert peer.read(100) == b'slotted socket'
    ctx.sslsocket_class = Custom
    raw = socket.socket()
    raw.settimeout(3)
    expected_allocation = (raw.family, raw.type, raw.proto, raw.fileno())
    peer = ctx.wrap_socket(raw, server_hostname='localhost')
    assert allocations[-1] == expected_allocation
    original_connect = socket.socket.connect
    def refused(*args):
        raise ConnectionRefusedError(errno.ECONNREFUSED, 'fixture connect failure')
    try:
        socket.socket.connect = refused
        try:
            peer.connect(('127.0.0.1', port))
        except ConnectionRefusedError:
            pass
        else:
            raise AssertionError('connect failure ignored')
    finally:
        socket.socket.connect = original_connect
    with peer:
        peer.connect(('127.0.0.1', port))
        peer.write(b'Eretry')
        assert peer.read(100) == b'retry'
    assert created == ['localhost'] * 3
    class Failed(ssl.SSLSocket):
        def __new__(cls, *args, **kwargs):
            raise RuntimeError('custom allocation failed')
    ctx.sslsocket_class = Failed
    raw = socket.socket()
    try:
        try:
            ctx.wrap_socket(raw, server_hostname='localhost')
        except RuntimeError:
            pass
        else:
            raise AssertionError('failing socket class initialized')
        assert raw.fileno() >= 0
    finally:
        raw.close()
    replacements = []
    class ClosedDuringSetup(ssl.SSLSocket):
        def settimeout(self, timeout):
            original_fd = socket.socket.fileno(self)
            socket.socket._real_close(self)
            replacement = socket.socket()
            replacements.append(replacement)
            assert replacement.fileno() == original_fd
            raise RuntimeError('closed during socket setup')
    ctx.sslsocket_class = ClosedDuringSetup
    raw = socket.socket()
    try:
        try:
            ctx.wrap_socket(raw, server_hostname='localhost')
        except RuntimeError:
            pass
        else:
            raise AssertionError('closed socket setup initialized')
        assert raw.fileno() == -1 and len(replacements) == 1
        replacements[0].gettimeout()
        assert replacements[0].getsockopt(socket.SOL_SOCKET, socket.SO_TYPE) == socket.SOCK_STREAM
    finally:
        raw.close()
        for replacement in replacements:
            replacement.close()
    for rejected in ('_io_refs', '_closed'):
        replacements = []
        class ClosedDuringInit(ssl.SSLSocket):
            def __setattr__(self, name, value):
                if name == rejected:
                    original_fd = socket.socket.fileno(self)
                    socket.socket._real_close(self)
                    replacement = socket.socket()
                    replacements.append(replacement)
                    assert replacement.fileno() == original_fd
                    raise RuntimeError('closed during native socket initialization')
                super().__setattr__(name, value)
        ctx.sslsocket_class = ClosedDuringInit
        raw = socket.socket()
        try:
            try:
                ctx.wrap_socket(raw, server_hostname='localhost')
            except RuntimeError:
                pass
            else:
                raise AssertionError('closed native socket initialized')
            assert len(replacements) == 1
            assert replacements[0].getsockopt(socket.SOL_SOCKET, socket.SO_TYPE) == socket.SOCK_STREAM
        finally:
            # Native ssl detaches after Python socket.__init__; if that fails,
            # raw can still refer to the already closed and reused descriptor.
            if replacements and raw.fileno() == replacements[0].fileno():
                raw.detach()
            else:
                raw.close()
            for replacement in replacements:
                replacement.close()
    if hasattr(ssl.SSLObject, 'close'):
        original_close = ssl.SSLObject.close
        closed = []
        def tracked_close(obj):
            if obj._connection is not None:
                original_close(obj)
                closed.append(obj)
        ssl.SSLObject.close = tracked_close
        try:
            for rejected in ('_sslobj', '_context', '_reading_header'):
                for failure in (RuntimeError, KeyboardInterrupt):
                    class FailedSetup(ssl.SSLSocket):
                        def __setattr__(self, name, value):
                            if name == rejected:
                                raise failure('custom setup failed')
                            super().__setattr__(name, value)
                    ctx.sslsocket_class = FailedSetup
                    raw = socket.socket()
                    before = len(closed)
                    retained = None
                    try:
                        try:
                            ctx.wrap_socket(raw, server_hostname='localhost')
                        except failure as error:
                            retained = error
                        else:
                            raise AssertionError('failing socket setup initialized')
                        assert retained is not None and retained.__traceback__ is not None
                        assert raw.fileno() >= 0
                        assert len(closed) == before + 1 and closed[-1]._connection is None
                    finally:
                        raw.close()
            retained_failures = []
            for slotted, rejected in ((slotted, name) for slotted in (False, True)
                                      for name in deferred_fields):
                for failure in (RuntimeError, KeyboardInterrupt):
                    class FailedConnectSetup(ssl.SSLSocket):
                        __slots__ = deferred_fields if slotted else ()
                        def __setattr__(self, name, value):
                            if getattr(self, 'armed', False) and name == rejected:
                                raise failure('deferred socket setup failed')
                            super().__setattr__(name, value)
                    ctx.sslsocket_class = FailedConnectSetup
                    raw = socket.socket()
                    raw.settimeout(3)
                    peer = ctx.wrap_socket(raw, server_hostname='localhost')
                    saved = {name: object.__getattribute__(peer, name) for name in deferred_fields}
                    before = len(closed)
                    peer.armed = True
                    try:
                        try:
                            peer.connect(('127.0.0.1', port))
                        except failure as error:
                            retained_failures.append(error)
                        else:
                            raise AssertionError('failing deferred setup initialized')
                        assert retained_failures[-1].__traceback__ is not None
                        assert len(closed) == before + 1 and closed[-1]._connection is None
                        failed_obj = closed[-1]
                        assert all(object.__getattribute__(peer, name) is value
                                   for name, value in saved.items())
                        assert peer.fileno() >= 0 and raw.fileno() == -1
                        peer.armed = False
                        peer.connect(('127.0.0.1', port))
                        peer.write(b'Edeferred retry')
                        assert peer.read(100) == b'deferred retry'
                    finally:
                        peer.armed = False
                        peer.close()
                    assert sum(obj is failed_obj for obj in closed) == 1
        finally:
            ssl.SSLObject.close = original_close
    return 'custom-socket-classes'


def socket_ca_directory(cert_dir, port, temp_dir):
    import os
    directory = temp_dir + '/socket-capath'
    os.mkdir(directory)
    with open(cert_dir + '/server.crt', 'rb') as source:
        data = source.read()
    with open(directory + '/ce275665.0', 'wb') as file:
        file.write(data)
    for tls12 in (False, True):
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        ctx.load_verify_locations(capath=directory)
        if tls12:
            ctx.maximum_version = ssl.TLSVersion.TLSv1_2
        assert ctx.cert_store_stats()['x509'] == 0
        saved = None
        for resumed in (False, True):
            raw = socket.create_connection(('127.0.0.1', port), timeout=3)
            with ctx.wrap_socket(raw, server_hostname='localhost', session=saved) as peer:
                assert peer.session_reused is resumed
                peer.sendall(b'Edirectory socket')
                response = b''
                while len(response) < len(b'directory socket'):
                    response += peer.recv(100)
                assert response == b'directory socket'
                saved = peer.session
            assert ctx.cert_store_stats() == {'x509': 1, 'x509_ca': 1, 'crl': 0}
            assert len(ctx.get_ca_certs()) == 1
            assert ctx.session_stats()['hits'] >= int(resumed)
    return 'socket-ca-directory'


def socket_generic_roles(cert_dir, port):
    for tls12 in (False, True):
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS)
        ctx.verify_mode = ssl.CERT_REQUIRED
        ctx.load_verify_locations(cafile=cert_dir + '/server.crt')
        if tls12:
            ctx.maximum_version = ssl.TLSVersion.TLSv1_2
        saved = None
        for attempt in range(2):
            server_object = ctx.wrap_bio(ssl.MemoryBIO(), ssl.MemoryBIO(), server_side=True)
            try:
                raw = socket.create_connection(('127.0.0.1', port), timeout=3)
                with ctx.wrap_socket(raw, server_hostname='localhost', session=saved) as peer:
                    assert peer.session_reused is (attempt > 0)
                    peer.sendall(b'Egeneric roles session')
                    response = b''
                    while len(response) < len(b'generic roles session'):
                        response += peer.recv(100)
                    assert response == b'generic roles session'
                    saved = peer.session
                stats = ctx.session_stats()
                assert stats['connect_good'] == attempt + 1 and stats['hits'] == attempt
                assert ctx.cert_store_stats()['x509'] == 1
            finally:
                close = getattr(server_object, 'close', None)
                if close is not None:
                    close()
    return 'socket-generic-roles'


def socket_live_credentials(cert_dir, port):
    for tls12 in (False, True):
        ctx = ssl.create_default_context(cafile=cert_dir + '/server.crt')
        if tls12:
            ctx.maximum_version = ssl.TLSVersion.TLSv1_2
        saved = None
        for number, name in enumerate(('server', 'client-leaf', 'ec')):
            ctx.load_cert_chain(cert_dir + '/' + name + '.crt',
                                cert_dir + ('/ec.key' if name == 'ec' else '/server.key'))
            raw = socket.create_connection(('127.0.0.1', port), timeout=3)
            with ctx.wrap_socket(raw, server_hostname='localhost', session=saved) as peer:
                assert peer.session_reused is (number > 0)
                peer.sendall(b'Elive credential session')
                response = b''
                while len(response) < len(b'live credential session'):
                    response += peer.recv(100)
                assert response == b'live credential session'
                saved = peer.session
            stats = ctx.session_stats()
            assert stats['connect_good'] == number + 1 and stats['hits'] == number
            assert ctx.cert_store_stats()['x509'] == 1
    return 'socket-live-credentials'


def socket_live_alpn(cert_dir, port):
    for tls12 in (False, True):
        ctx = ssl.create_default_context(cafile=cert_dir + '/server.crt')
        if tls12:
            ctx.maximum_version = ssl.TLSVersion.TLSv1_2
        saved = None
        for number, protocols in enumerate((['http/1.1'], ['h2', 'http/1.1'], ['http/1.1'])):
            ctx.set_alpn_protocols(protocols)
            raw = socket.create_connection(('127.0.0.1', port), timeout=3)
            with ctx.wrap_socket(raw, server_hostname='localhost', session=saved) as peer:
                assert peer.session_reused is (number > 0)
                assert peer.selected_alpn_protocol() == 'http/1.1'
                peer.sendall(b'Elive ALPN session')
                response = b''
                while len(response) < len(b'live ALPN session'):
                    response += peer.recv(100)
                assert response == b'live ALPN session'
                saved = peer.session
            stats = ctx.session_stats()
            assert stats['connect_good'] == number + 1 and stats['hits'] == number
            assert ctx.cert_store_stats()['x509'] == 1
    return 'socket-live-alpn'


def socket_live_key_exchange(cert_dir, port):
    for tls12 in (False, True):
        ctx = ssl.create_default_context(cafile=cert_dir + '/server.crt')
        if tls12:
            ctx.maximum_version = ssl.TLSVersion.TLSv1_2
        saved = None
        for number, field in enumerate(('initial', 'dh', 'curve256', 'curve384')):
            if field == 'dh':
                ctx.load_dh_params(cert_dir + '/dh2048.pem')
            elif field.startswith('curve'):
                ctx.set_ecdh_curve('prime256v1' if field == 'curve256' else 'secp384r1')
            raw = socket.create_connection(('127.0.0.1', port), timeout=3)
            with ctx.wrap_socket(raw, server_hostname='localhost', session=saved) as peer:
                assert peer.session_reused is (number > 0)
                peer.sendall(b'Elive key exchange session')
                response = b''
                while len(response) < len(b'live key exchange session'):
                    response += peer.recv(100)
                assert response == b'live key exchange session'
                saved = peer.session
            stats = ctx.session_stats()
            assert stats['connect_good'] == number + 1 and stats['hits'] == number
            assert ctx.cert_store_stats()['x509'] == 1
    return 'socket-live-key-exchange'


def socket_live_tls13(cert_dir, port):
    for tls12 in (False, True):
        ctx = ssl.create_default_context(cafile=cert_dir + '/server.crt')
        if tls12:
            ctx.maximum_version = ssl.TLSVersion.TLSv1_2
        saved = None
        for number, enabled in enumerate((False, True, False)):
            ctx.post_handshake_auth = enabled
            raw = socket.create_connection(('127.0.0.1', port), timeout=3)
            with ctx.wrap_socket(raw, server_hostname='localhost', session=saved) as peer:
                assert peer.session_reused is (number > 0)
                peer.sendall(b'Elive TLS13 session')
                response = b''
                while len(response) < len(b'live TLS13 session'):
                    response += peer.recv(100)
                assert response == b'live TLS13 session'
                saved = peer.session
            stats = ctx.session_stats()
            assert stats['connect_good'] == number + 1 and stats['hits'] == number
            assert ctx.cert_store_stats()['x509'] == 1
    return 'socket-live-tls13'


def socket_live_settings(cert_dir, port):
    for tls12 in (False, True):
        ctx = ssl.create_default_context(cafile=cert_dir + '/server.crt')
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE
        if tls12:
            ctx.maximum_version = ssl.TLSVersion.TLSv1_2
        fields = ('initial', 'check-on', 'verify-optional', 'verify-required')
        if not tls12:
            fields += ('minimum', 'maximum')
        saved = None
        for number, field in enumerate(fields):
            if field == 'check-on':
                ctx.check_hostname = True
                assert ctx.verify_mode == ssl.CERT_REQUIRED
            elif field == 'verify-optional':
                ctx.verify_mode = ssl.CERT_OPTIONAL
            elif field == 'verify-required':
                ctx.verify_mode = ssl.CERT_REQUIRED
            elif field == 'minimum':
                ctx.minimum_version = ssl.TLSVersion.TLSv1_3
            elif field == 'maximum':
                ctx.maximum_version = ssl.TLSVersion.TLSv1_3
            raw = socket.create_connection(('127.0.0.1', port), timeout=3)
            with ctx.wrap_socket(raw, server_hostname='localhost', session=saved) as peer:
                assert peer.session_reused is (number > 0)
                peer.sendall(b'Elive settings session')
                response = b''
                while len(response) < len(b'live settings session'):
                    response += peer.recv(100)
                assert response == b'live settings session'
                saved = peer.session
            stats = ctx.session_stats()
            assert stats['connect_good'] == number + 1 and stats['hits'] == number
            assert ctx.cert_store_stats()['x509'] == 1
    return 'socket-live-settings'


def socket_live_policy(cert_dir, port):
    for tls12 in (False, True):
        ctx = ssl.create_default_context(cafile=cert_dir + '/server.crt')
        if tls12:
            ctx.maximum_version = ssl.TLSVersion.TLSv1_2
        saved = None
        for number, field in enumerate(('initial', 'options', 'verify-flags', 'common-name', 'ciphers')):
            if field == 'options':
                ctx.options ^= ssl.OP_CIPHER_SERVER_PREFERENCE
            elif field == 'verify-flags':
                ctx.verify_flags ^= ssl.VERIFY_X509_TRUSTED_FIRST
            elif field == 'common-name':
                ctx.hostname_checks_common_name = False
            elif field == 'ciphers':
                ctx.set_ciphers('ECDHE-RSA-AES256-GCM-SHA384:ECDHE-RSA-AES128-GCM-SHA256')
            raw = socket.create_connection(('127.0.0.1', port), timeout=3)
            with ctx.wrap_socket(raw, server_hostname='localhost', session=saved) as peer:
                assert peer.session_reused is (number > 0)
                peer.sendall(b'Elive policy session')
                response = b''
                while len(response) < len(b'live policy session'):
                    response += peer.recv(100)
                assert response == b'live policy session'
                saved = peer.session
            stats = ctx.session_stats()
            assert stats['connect_good'] == number + 1 and stats['hits'] == number
            assert ctx.cert_store_stats()['x509'] == 1
    return 'socket-live-policy'


def socket_live_trust(cert_dir, port, temp_dir):
    import os
    directory = temp_dir + '/socket-live-trust'
    os.mkdir(directory)
    with open(cert_dir + '/server.crt', 'rb') as source:
        data = source.read()
    for tls12 in (False, True):
        with open(directory + '/ce275665.0', 'wb') as file:
            file.write(data)
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        ctx.load_verify_locations(capath=directory)
        if tls12:
            ctx.maximum_version = ssl.TLSVersion.TLSv1_2
        saved = None
        for number in range(3):
            raw = socket.create_connection(('127.0.0.1', port), timeout=3)
            with ctx.wrap_socket(raw, server_hostname='localhost', session=saved) as peer:
                assert peer.session_reused is (number > 0)
                peer.sendall(b'Elive store session')
                response = b''
                while len(response) < len(b'live store session'):
                    response += peer.recv(100)
                assert response == b'live store session'
                saved = peer.session
            if number == 0:
                os.unlink(directory + '/ce275665.0')
                ctx.load_verify_locations(cafile=cert_dir + '/ca.crt')
            else:
                ctx.load_verify_locations(cafile=cert_dir + '/ca.crt')
            assert ctx.cert_store_stats()['x509'] == 2
            stats = ctx.session_stats()
            assert stats['connect_good'] == number + 1
            assert stats['hits'] == number
        # A fresh handshake still trusts the anchor cached before removal.
        raw = socket.create_connection(('127.0.0.1', port), timeout=3)
        with ctx.wrap_socket(raw, server_hostname='localhost') as peer:
            assert peer.session_reused is False
        assert ctx.cert_store_stats()['x509'] == 2
        assert ctx.session_stats()['connect_good'] == 4
    return 'socket-live-trust'


def socket_sendfile(cert_dir, port, temp_dir):
    import io
    import os
    payload = b'guest TLS file bytes\x00\xff' * 900
    path = temp_dir + '/sendfile-data.bin'
    with open(path, 'wb') as file:
        file.write(payload)
    def receive(peer, expected):
        actual = b''
        while len(actual) < len(expected):
            part = peer.recv(min(4096, len(expected) - len(actual)))
            assert part
            actual += part
        assert actual == expected
    try:
        for offset, count, initial in ((0, None, 0), (7, 9001, 0), (0, None, 3),
                                       (len(payload) + 1, 10, 0), (2, True, 0)):
            with connect(cert_dir, port) as peer, open(path, 'rb') as file:
                peer.sendall(b'E')
                file.seek(initial)
                start = offset if offset else initial
                expected = payload[start:] if count is None else payload[start:start + count]
                assert peer.sendfile(file, offset, count) == len(expected)
                assert file.tell() == (offset + len(expected) if expected else start)
                receive(peer, expected)
        with connect(cert_dir, port) as peer:
            peer.sendall(b'E')
            file = io.BytesIO(b'memory file')
            assert peer.sendfile(file, 2, 5) == 5 and file.tell() == 7
            receive(peer, b'mory ')
            for count, kind in ((0, ValueError), (-1, ValueError), (False, ValueError),
                                 (1.0, TypeError), ('1', TypeError)):
                try:
                    peer.sendfile(io.BytesIO(b'data'), count=count)
                except kind:
                    pass
                else:
                    raise AssertionError('invalid sendfile count accepted')
            with open(path, 'r') as text:
                try:
                    peer.sendfile(text)
                except ValueError:
                    pass
                else:
                    raise AssertionError('text file accepted')
            peer.setblocking(False)
            file = io.BytesIO(b'unchanged')
            try:
                peer.sendfile(file)
            except ValueError:
                pass
            else:
                raise AssertionError('nonblocking sendfile accepted')
            assert file.tell() == 0
        class PartialSocket(ssl.SSLSocket):
            def send(self, data, flags=0):
                if self.abort and self.accepted:
                    raise OSError('injected send failure')
                sent = super().send(memoryview(data)[:7], flags)
                self.accepted += sent
                return sent
        ctx = context(cert_dir)
        ctx.sslsocket_class = PartialSocket
        for abort in (False, True):
            raw = socket.create_connection(('127.0.0.1', port), timeout=3)
            with ctx.wrap_socket(raw, server_hostname='localhost') as peer:
                peer.abort, peer.accepted = False, 0
                peer.sendall(b'E')
                peer.abort, peer.accepted = abort, 0
                file = io.BytesIO(b'0123456789' * 4)
                if abort:
                    try:
                        peer.sendfile(file, 3, 21)
                    except OSError:
                        pass
                    else:
                        raise AssertionError('sendfile hid partial send failure')
                    assert peer.accepted == 7 and file.tell() == 10
                    receive(peer, b'3456789')
                else:
                    assert peer.sendfile(file, 3, 21) == 21
                    assert peer.accepted == 21 and file.tell() == 24
                    receive(peer, b'345678901234567890123')
    finally:
        os.unlink(path)
    return 'socket-sendfile'


def socket_compression_and_npn(cert_dir, port):
    import warnings
    raw = socket.socket()
    peer = context(cert_dir).wrap_socket(raw, server_hostname='localhost', do_handshake_on_connect=False)
    try:
        for state in ('unconnected', 'before-handshake', 'connected', 'closed'):
            if state == 'before-handshake':
                peer.connect(('127.0.0.1', port))
            elif state == 'connected':
                peer.do_handshake()
            elif state == 'closed':
                peer.close()
            assert peer.compression() is None
            with warnings.catch_warnings(record=True) as seen:
                warnings.simplefilter('always')
                assert peer.selected_npn_protocol() is None
                assert len(seen) == 1 and seen[0].category is DeprecationWarning
    finally:
        peer.close()
    return 'socket-compression-and-npn'


def socket_messages(cert_dir, port):
    for tls12 in (False, True):
        events = []
        ctx = context(cert_dir)
        if tls12:
            ctx.maximum_version = ssl.TLSVersion.TLSv1_2
        ctx._msg_callback = lambda *args: events.append(args)
        raw = socket.create_connection(('127.0.0.1', port), timeout=3)
        with ctx.wrap_socket(raw, server_hostname='localhost') as peer:
            peer.sendall(b'Ediagnostics')
            assert peer.recv(100) == b'diagnostics'
            assert events and all(event[0] is peer for event in events)
            assert any(event[3] == ssl._TLSContentType.HANDSHAKE and
                       event[4] == ssl._TLSMessageType.FINISHED for event in events)
            assert all(event[3] != ssl._TLSContentType.APPLICATION_DATA and
                       b'Ediagnostics' not in event[5] for event in events)
    return 'socket-messages'


def socket_message_failures(cert_dir, port):
    errors = []
    def fail(*args):
        error = RuntimeError('message callback ' + str(len(errors)))
        errors.append(error)
        raise error
    for nonblocking in (False, True):
        ctx = context(cert_dir)
        ctx._msg_callback = fail
        raw = socket.create_connection(('127.0.0.1', port), timeout=3)
        with ctx.wrap_socket(raw, server_hostname='localhost', do_handshake_on_connect=False) as peer:
            errors.clear()
            if nonblocking:
                peer.setblocking(False)
                try:
                    peer.do_handshake()
                except ssl.SSLWantReadError as error:
                    assert errors and error.__context__ is errors[-1]
                else:
                    raise AssertionError('callback replaced socket readiness')
                ctx._msg_callback = lambda *args: None
                peer.settimeout(3)
                peer.do_handshake()
            else:
                try:
                    peer.do_handshake()
                except RuntimeError as error:
                    assert errors and error is errors[-1]
                else:
                    raise AssertionError('blocking handshake hid callback failure')
                ctx._msg_callback = lambda *args: None
            ctx._msg_callback = fail
            errors.clear()
            try:
                peer.sendall(b'Eone completed write')
            except RuntimeError as error:
                assert errors and error is errors[-1]
            else:
                raise AssertionError('socket write hid callback failure')
            ctx._msg_callback = None
            assert peer.recv(100) == b'one completed write'
    return 'socket-message-failures'


def socket_message_guards(cert_dir, port):
    ctx = context(cert_dir)
    events, errors = [], []
    def callback(peer, *args):
        if not hasattr(peer._sslobj, '_connection'):
            return
        events.append(args)
        if len(events) == 1:
            import io
            file = io.BytesIO(b'guest file data')
            fd, timeout = peer.fileno(), peer.gettimeout()
            engine = peer._sslobj
            deferred = engine._defer_message_errors
            queued = peer._outgoing.pending
            for call in (lambda: peer.read(1), lambda: peer.write(b'nested'), peer.close,
                         lambda: peer.sendall(b''), lambda: peer.do_handshake(block=True),
                         lambda: peer.sendfile(file, offset=2)):
                try:
                    call()
                except ssl.SSLError:
                    pass
                else:
                    raise AssertionError('nested TLS operation was permitted')
                assert peer.fileno() == fd and not peer._closed
                assert peer.gettimeout() == timeout and engine._defer_message_errors == deferred
                assert peer._outgoing.pending == queued and file.tell() == 0
            return
        error = RuntimeError('guarded message callback')
        errors.append(error)
        raise error
    ctx._msg_callback = callback
    raw = socket.create_connection(('127.0.0.1', port), timeout=3)
    with ctx.wrap_socket(raw, server_hostname='localhost', do_handshake_on_connect=False) as peer:
        if not hasattr(peer._sslobj, '_connection'):
            return 'socket-message-guards'
        try:
            peer.do_handshake()
        except RuntimeError as error:
            assert errors and error is errors[-1]
        else:
            raise AssertionError('blocking handshake hid later callback failure')
        assert peer.version() is not None and peer.gettimeout() == 3
        ctx._msg_callback = None
        peer.sendall(b'Eguarded socket')
        assert peer.recv(100) == b'guarded socket'
    return 'socket-message-guards'


def socket_keylog(cert_dir, port, temp_dir):
    import os
    for tls12 in (False, True):
        ctx = context(cert_dir)
        if tls12:
            ctx.maximum_version = ssl.TLSVersion.TLSv1_2
        path = temp_dir + '/socket-keylog.log'
        try:
            os.unlink(path)
        except FileNotFoundError:
            pass
        ctx.keylog_filename = path
        try:
            raw = socket.create_connection(('127.0.0.1', port), timeout=3)
            with ctx.wrap_socket(raw, server_hostname='localhost') as peer:
                peer.sendall(b'Ekeylog')
                assert peer.recv(100) == b'keylog', 'keylog first response'
                session = peer.session
            ctx.keylog_filename = None
            ctx.keylog_filename = path
            raw = socket.create_connection(('127.0.0.1', port), timeout=3)
            with ctx.wrap_socket(raw, server_hostname='localhost', session=session) as peer:
                assert peer.session_reused, 'keylog session not reused: ' + str(tls12)
                peer.sendall(b'Eresumed')
                assert peer.recv(100) == b'resumed', 'keylog resumed response'
            with open(path, 'rb') as file:
                lines = [line.split() for line in file.read().splitlines() if line and not line.startswith(b'#')]
            assert lines and all(len(parts) == 3 and len(parts[1]) == 64 for parts in lines), 'keylog line format'
            labels = {parts[0] for parts in lines}
            assert (b'CLIENT_RANDOM' if tls12 else b'CLIENT_TRAFFIC_SECRET_0') in labels, 'keylog label missing: ' + str(tls12)
        finally:
            ctx.keylog_filename = None
    return 'socket-keylog'


def post_auth_socket_states(cert_dir, port):
    def expects(peer, kind):
        try:
            peer.verify_client_post_handshake()
        except kind:
            return
        raise AssertionError('unexpected post-handshake request state')
    raw = socket.socket()
    peer = context(cert_dir).wrap_socket(raw, server_hostname='localhost', do_handshake_on_connect=False)
    try:
        expects(peer, ValueError)
    finally:
        peer.close()
    expects(peer, ValueError)
    with connect(cert_dir, port, do_handshake_on_connect=False) as peer:
        expects(peer, ssl.SSLError)
    raw = socket.socket()
    raw.setblocking(False)
    peer = context(cert_dir).wrap_socket(raw, server_hostname='localhost', do_handshake_on_connect=False)
    try:
        status = peer.connect_ex(('127.0.0.1', port))
        assert status in (0, errno.EINPROGRESS, errno.EWOULDBLOCK, errno.EALREADY), status
        expects(peer, ssl.SSLError)
    finally:
        peer.close()
    with connect(cert_dir, port) as peer:
        peer.sendall(b'U')
        plain = peer.unwrap()
        try:
            expects(peer, ValueError)
            assert plain.recv(100) == b'plain-after-tls'
        finally:
            plain.close()
    return 'post-auth-socket-states'


def nonblocking_and_timeout(cert_dir, port):
    with connect(cert_dir, port, do_handshake_on_connect=False) as peer:
        peer.setblocking(False)
        deadline = time.monotonic() + 3
        while True:
            try:
                peer.do_handshake()
                break
            except (ssl.SSLWantReadError, ssl.SSLWantWriteError):
                assert time.monotonic() < deadline
                time.sleep(0.001)
        # Discard session tickets while waiting for actual application data.
        try:
            peer.recv(1)
        except ssl.SSLWantReadError:
            pass
        else:
            raise AssertionError("nonblocking recv must request readiness")
        peer.settimeout(0.05)
        peer.sendall(b"T")
        start = time.monotonic()
        try:
            peer.recv(1)
        except TimeoutError:
            elapsed = time.monotonic() - start
            assert 0.01 <= elapsed < 1
        else:
            raise AssertionError("missing TLS read timeout")
        assert peer.gettimeout() == 0.05
    return "nonblocking-and-timeout"


def nonblocking_exchange(cert_dir, port):
    data = b"partial-tls-write" * 100000
    with connect(cert_dir, port) as peer:
        peer.sendall(b"E")
        peer.setsockopt(socket.SOL_SOCKET, socket.SO_SNDBUF, 4096)
        peer.setblocking(False)
        sent = 0
        received = bytearray()
        deadline = time.monotonic() + 10
        while len(received) < len(data):
            assert time.monotonic() < deadline
            if sent < len(data):
                try:
                    sent += peer.send(memoryview(data)[sent:sent + 262144])
                except (ssl.SSLWantReadError, ssl.SSLWantWriteError):
                    pass
            try:
                chunk = peer.recv(65536)
                assert chunk
                received.extend(chunk)
            except (ssl.SSLWantReadError, ssl.SSLWantWriteError):
                time.sleep(0.001)
        assert sent == len(data) and bytes(received) == data
    return len(data)


def switch_to_blocking_with_pending_write(cert_dir, port):
    with connect(cert_dir, port) as peer:
        peer.sendall(b"E" + b"ready-output" * 10)
        assert peer.recv(1) == b"r"
        peer.setsockopt(socket.SOL_SOCKET, socket.SO_SNDBUF, 4096)
        peer.setblocking(False)
        data = b"queued-output" * 20000
        deadline = time.monotonic() + 3
        while True:
            assert time.monotonic() < deadline
            try:
                peer.send(data)
            except ssl.SSLWantWriteError:
                break
        time.sleep(0.05)
        peer.settimeout(0.2)
        # The peer has echoed input and may also be blocked on its output.
        # Reading must work while our ciphertext remains queued.
        assert peer.recv(64)
        assert peer.gettimeout() == 0.2
    return "read-with-pending-write"


def coalesced_unwrap(cert_dir, port):
    with connect(cert_dir, port) as peer:
        peer.sendall(b"U")
        plain = peer.unwrap()
        # Detach too: no Python-side buffer can substitute for the descriptor.
        with socket.socket(fileno=plain.detach()) as detached:
            assert detached.recv(100) == b"plain-after-tls"
    return "coalesced-close-and-plaintext"


def write_timeout(cert_dir, port):
    with connect(cert_dir, port) as peer:
        peer.sendall(b"T")
        peer.setsockopt(socket.SOL_SOCKET, socket.SO_SNDBUF, 4096)
        peer.settimeout(0.05)
        start = time.monotonic()
        try:
            peer.sendall(b"slow-reader" * 300000)
        except TimeoutError:
            assert 0.01 <= time.monotonic() - start < 0.5
        else:
            raise AssertionError("missing overall TLS write timeout")
        assert peer.gettimeout() == 0.05
    return "overall-write-timeout"


def read_completion_after_deadline(cert_dir, port):
    """Guest fault injection: delay TLS work after it has consumed plaintext."""
    with connect(cert_dir, port) as peer:
        peer.sendall(b"Edeadline-plaintext")
        assert peer.recv(1) == b"d"
        read = peer._sslobj.read

        def delayed_read(*args):
            result = read(*args)
            time.sleep(0.03)
            return result

        peer._sslobj.read = delayed_read
        peer.settimeout(0.01)
        assert peer.recv(100) == b"eadline-plaintext"
        assert peer.gettimeout() == 0.01
    return "late-read-preserved"


def asyncio_tls_client(cert_dir, port, tls12=False, host='127.0.0.1'):
    import asyncio
    context = ssl.create_default_context(cafile=cert_dir + '/server.crt')
    context.minimum_version = context.maximum_version = (
        ssl.TLSVersion.TLSv1_2 if tls12 else ssl.TLSVersion.TLSv1_3)
    async def exchange():
        reader, writer = await asyncio.open_connection(
            host, port, ssl=context, server_hostname='localhost',
            ssl_handshake_timeout=3, ssl_shutdown_timeout=3)
        try:
            obj = writer.get_extra_info('ssl_object')
            assert obj.version() == ('TLSv1.2' if tls12 else 'TLSv1.3')
            assert writer.get_extra_info('peercert')['subject']
            data = b'asyncio-loopback-' * 32768
            writer.write(b'E')
            writer.write(memoryview(data))
            await writer.drain()
            assert await asyncio.wait_for(reader.readexactly(len(data)), 5) == data
        finally:
            writer.close()
            await writer.wait_closed()
        try:
            await asyncio.open_connection(
                host, port, ssl=context, server_hostname='wrong.example',
                ssl_handshake_timeout=3, ssl_shutdown_timeout=3)
        except ssl.SSLCertVerificationError as error:
            assert error.verify_code == 62
        else:
            raise AssertionError('asyncio accepted the wrong TLS hostname')
    asyncio.run(exchange())
    return 'asyncio-tls-client'


def asyncio_dns_denied(port):
    import asyncio
    async def lookup():
        try:
            await asyncio.get_running_loop().getaddrinfo(
                'localhost', port, type=socket.SOCK_STREAM)
        except socket.gaierror:
            return 'asyncio-dns-denied'
        raise AssertionError('asyncio bypassed the WASI DNS permission')
    return asyncio.run(lookup())


def eof_and_plain_peer(cert_dir, port, plain_port):
    with connect(cert_dir, port) as peer:
        peer.sendall(b"R")
        assert peer.recv(1) == b""
    with connect(cert_dir, port, suppress_ragged_eofs=False) as peer:
        peer.sendall(b"R")
        try:
            peer.recv(1)
        except ssl.SSLEOFError as error:
            assert error.library == 'SSL'
            assert error.reason == 'UNEXPECTED_EOF_WHILE_READING'
        else:
            raise AssertionError("missing unexpected EOF")
    try:
        connect(cert_dir, plain_port)
    except (ssl.SSLError, ConnectionError):
        pass
    else:
        raise AssertionError("plaintext peer accepted as TLS")
    return "eof-and-plain-peer"


def denied_connect(port):
    try:
        socket.create_connection(("127.0.0.1", port), timeout=0.5)
    except PermissionError:
        return "network-denied"
    raise AssertionError("WASI network capability was bypassed")


def certificate_rejections(cert_dir, port):
    for ctx, hostname in ((context(cert_dir), "wrong.example"),
                          (context(cert_dir), "127.0.0.2"),
                          (ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT), "localhost")):
        with socket.create_connection(("127.0.0.1", port), timeout=3) as raw:
            try:
                ctx.wrap_socket(raw, server_hostname=hostname)
            except ssl.SSLCertVerificationError as error:
                assert error.library == 'SSL'
                assert error.reason == 'CERTIFICATE_VERIFY_FAILED'
                expected = {
                    'wrong.example': "Hostname mismatch, certificate is not valid for 'wrong.example'.",
                    '127.0.0.2': "IP address mismatch, certificate is not valid for '127.0.0.2'.",
                    'localhost': 'self-signed certificate',
                }
                assert error.verify_message == expected[hostname]
            else:
                raise AssertionError("untrusted certificate accepted")
    return "socket-certificate-rejections"


def unsupported_io(cert_dir, port):
    with connect(cert_dir, port) as peer:
        for operation in (lambda: peer.send(b"raw", 1), lambda: peer.recv(1, 1),
                          lambda: peer.sendto(b"raw", ("127.0.0.1", port)),
                          lambda: peer.dup()):
            try:
                operation()
            except (ValueError, NotImplementedError):
                pass
            else:
                raise AssertionError("operation bypassed TLS")
    return "unsupported-io-rejected"


def server_certificate_helpers(cert_dir, port):
    with open(cert_dir + "/server.crt") as file:
        expected = file.read()
    for ca in (None, cert_dir + "/server.crt"):
        assert ssl.get_server_certificate(("localhost", port), ca_certs=ca, timeout=3) == expected
    with connect(cert_dir, port) as peer:
        decoded = peer.getpeercert()
        assert decoded["subject"] == ((("commonName", "localhost"),),)
        assert ("DNS", "localhost") in decoded["subjectAltName"]
    return "server-certificate-helpers"


def socket_connection_metadata(cert_dir, port):
    with connect(cert_dir, port, do_handshake_on_connect=False) as peer:
        assert peer.get_channel_binding() is None and peer.shared_ciphers() is None
        peer.do_handshake()
        binding = peer.get_channel_binding()
        assert isinstance(binding, bytes) and len(binding) in (12, 32, 48)
        assert peer.shared_ciphers() is None
        peer.sendall(b"B")
        wire = b""
        while len(wire) < len(binding) + 2:
            chunk = peer.recv(3)
            assert chunk
            wire += chunk
        assert int.from_bytes(wire[:2], "big") == len(binding) and wire[2:] == binding
        plain = peer.unwrap()
        assert peer.get_channel_binding() is None and peer.shared_ciphers() is None
        for kind in ("tls-exporter", b"tls-unique", None):
            try:
                peer.get_channel_binding(kind)
            except ValueError:
                pass
            else:
                raise AssertionError("unwrapped socket accepted unsupported binding type")
        plain.close()
    return "socket-connection-metadata"


def socket_session_resumption(cert_dir, port, tls12=False, unconnected=False):
    context = ssl.create_default_context(cafile=cert_dir + "/server.crt")
    if tls12:
        context.maximum_version = ssl.TLSVersion.TLSv1_2
    saved = None
    for attempt in range(5 if unconnected else 2):
        raw = socket.socket() if unconnected else socket.create_connection(("127.0.0.1", port), timeout=3)
        raw.settimeout(3)
        with context.wrap_socket(raw, server_hostname="localhost", session=saved) as peer:
            if unconnected:
                assert peer.session is None and peer.session_reused is None
                if attempt in (3, 4):
                    operation = "connect" if attempt == 3 else "connect_ex"
                    original = getattr(socket.socket, operation)
                    def refused(self, address):
                        raise ConnectionRefusedError(errno.ECONNREFUSED, "injected TCP connection refusal")
                    setattr(socket.socket, operation, refused)
                    try:
                        try:
                            getattr(peer, operation)(("127.0.0.1", port))
                        except ConnectionRefusedError:
                            pass
                        else:
                            raise AssertionError("TCP connection refusal was not propagated")
                    finally:
                        setattr(socket.socket, operation, original)
                    assert peer.session is None
                peer.session = None
                if attempt < 2:
                    peer.session = saved
                if attempt == 4:
                    assert peer.connect_ex(("127.0.0.1", port)) == 0
                else:
                    peer.connect(("127.0.0.1", port))
            assert peer.session_reused is (attempt == 1)
            binding = peer.get_channel_binding()
            peer.sendall(b"B")
            wire = b""
            while len(wire) < len(binding) + 2:
                chunk = peer.recv(3)
                assert chunk
                wire += chunk
            assert int.from_bytes(wire[:2], "big") == len(binding) and wire[2:] == binding
            saved = peer.session
            assert isinstance(saved, ssl.SSLSession) and saved.has_ticket
            plain = peer.unwrap()
            assert peer.session is None and peer.session_reused is None
            peer.session = None
            peer.session = saved
            plain.close()
    assert saved.id and saved.timeout > 0
    return "socket-session-resumption"


def main():
    from concurrent.futures import ThreadPoolExecutor
    from ssl_socket_server import listener_client
    import json
    import pathlib
    import subprocess
    import sys
    import tempfile
    with tempfile.TemporaryDirectory() as directory:
        state = pathlib.Path(directory) / "server.json"
        server = subprocess.Popen([sys.executable, "guests/python/tests/integration/ssl_socket_server.py", str(state), sys.argv[1]])
        try:
            deadline = time.monotonic() + 5
            while not state.exists():
                if server.poll() is not None or time.monotonic() >= deadline:
                    raise RuntimeError("TLS test server did not start")
                time.sleep(0.01)
            peer = json.loads(state.read_text())
            print(private_socket_arguments())
            for tls12 in (False, True):
                print(private_socket_exchange(sys.argv[1], peer['tls_port'], tls12))
            print(private_socket_nonblocking(sys.argv[1], peer['tls_port']))
            print(private_socket_closed_transport(sys.argv[1], peer['tls_port']))
            print(private_socket_tracebacks(sys.argv[1], peer['tls_port'], peer['plain_port']))
            print(private_socket_shutdown(sys.argv[1], peer['coalesced_port']))
            for public in (False, True):
                with socket.socket() as reserve:
                    reserve.bind(('127.0.0.1', 0))
                    port = reserve.getsockname()[1]
                with ThreadPoolExecutor(max_workers=1) as executor:
                    client = executor.submit(listener_client, sys.argv[1], port)
                    print(private_socket_listener(sys.argv[1], port, public))
                    assert client.result(timeout=6) == 'listener-client-ok'
            print(requests_resources())
            print(requests_default_verification(peer["strict_port"]))
            print(socket_keylog(sys.argv[1], peer['tls_port'], directory))
            print(socket_buffer_rules(sys.argv[1], peer['tls_port']))
            print(socket_empty_write_handshake(sys.argv[1], peer['tls_port']))
            print(socket_pending_buffer_rules(sys.argv[1], peer['tls_port']))
            print(socket_unconnected_buffer_rules(sys.argv[1], peer['tls_port']))
            print(socket_messages(sys.argv[1], peer['tls_port']))
            print(socket_message_failures(sys.argv[1], peer['tls_port']))
            print(socket_message_guards(sys.argv[1], peer['tls_port']))
            print(socket_sendfile(sys.argv[1], peer['tls_port'], directory))
            print(socket_ca_directory(sys.argv[1], peer['tls_port'], directory))
            print(socket_live_trust(sys.argv[1], peer['tls_port'], directory))
            print(socket_live_policy(sys.argv[1], peer['tls_port']))
            print(socket_live_settings(sys.argv[1], peer['tls_port']))
            print(socket_live_tls13(sys.argv[1], peer['tls_port']))
            print(socket_live_key_exchange(sys.argv[1], peer['tls_port']))
            print(socket_live_alpn(sys.argv[1], peer['tls_port']))
            print(socket_live_credentials(sys.argv[1], peer['tls_port']))
            print(socket_generic_roles(sys.argv[1], peer['tls_port']))
            print(socket_compression_and_npn(sys.argv[1], peer['tls_port']))
            for tls12 in (False, True):
                print('asyncio client', tls12, asyncio_tls_client(sys.argv[1], peer['tls_port'], tls12))
                print('asyncio hostname', tls12,
                      asyncio_tls_client(sys.argv[1], peer['tls_port'], tls12, 'localhost'))
            for name in ("https_clients", "urllib3_clients", "urllib3_certificate_rejections", "requests_clients", "large_post", "deferred_handshake_and_unwrap", "post_auth_socket_states", "custom_socket_classes",
                         "nonblocking_and_timeout", "nonblocking_exchange",
                         "switch_to_blocking_with_pending_write", "coalesced_unwrap", "write_timeout", "eof_and_plain_peer",
                         "certificate_rejections", "unsupported_io", "server_certificate_helpers", "socket_connection_metadata"):
                args = [sys.argv[1], peer["tls_port"]]
                if name == "coalesced_unwrap":
                    args[1] = peer["coalesced_port"]
                if name.startswith(("urllib3_", "requests_")):
                    args[1] = peer["strict_port"]
                if name == "eof_and_plain_peer":
                    args.append(peer["plain_port"])
                print(name, globals()[name](*args))
            for tls12 in (False, True):
                for unconnected in (False, True):
                    print("socket session", tls12, unconnected,
                          socket_session_resumption(sys.argv[1], peer["tls_port"], tls12, unconnected))
            for deferred, reject, sni in ((False, False, False), (True, False, False), (False, True, False),
                                         (False, False, True), (True, False, True), (False, True, True),
                                         (True, False, 'pre-both'), (True, False, 'pre-enable'), (True, False, 'pre-disable'),
                                         (True, False, 'live-enable'), (True, False, 'live-disable'), (True, False, 'live-replace')):
                with socket.socket() as reserve:
                    reserve.bind(("127.0.0.1", 0))
                    port = reserve.getsockname()[1]
                with ThreadPoolExecutor(max_workers=1) as executor:
                    client = executor.submit(listener_client, sys.argv[1], port, "sni-alert-first" if sni and reject else reject)
                    print("listener", deferred, reject, sni, listener_exchange(sys.argv[1], port, deferred, reject, sni))
                    assert client.result(timeout=6) == "listener-client-ok"
            print(nonblocking_listener(sys.argv[1], 0))
            with socket.socket() as reserve:
                reserve.bind(('127.0.0.1', 0))
                port = reserve.getsockname()[1]
            with ThreadPoolExecutor(max_workers=1) as executor:
                client = executor.submit(listener_client, sys.argv[1], port, 'pha')
                print('listener PHA', listener_exchange(sys.argv[1], port, post_auth=True))
                assert client.result(timeout=6) == 'listener-client-ok'
        finally:
            server.terminate()
            server.wait(timeout=5)


if __name__ == "__main__":
    main()
