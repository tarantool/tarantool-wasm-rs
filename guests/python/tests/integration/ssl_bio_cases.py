"""Run unchanged with native CPython 3.12 or the guest SSL BIO adapter."""
import ssl
import sys


def tls_pair(cert_dir, name="localhost", trusted=True, maximum=None):
    client = ssl.create_default_context(cafile=cert_dir + "/server.crt") if trusted else ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    server = ssl.create_default_context(ssl.Purpose.CLIENT_AUTH)
    server.load_cert_chain(cert_dir + "/server.crt", cert_dir + "/server.key")
    client.set_alpn_protocols(["h2", "http/1.1"])
    server.set_alpn_protocols(["http/1.1", "h2"])
    if maximum is not None:
        client.maximum_version = server.maximum_version = maximum
    ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
    c = client.wrap_bio(ci, co, server_hostname=name)
    s = server.wrap_bio(si, so, server_side=True)
    return c, s, ci, co, si, so


def transfer(source, target):
    data = source.read()
    for offset in range(0, len(data), 7):
        target.write(data[offset:offset + 7])


def handshake(pair):
    c, s, ci, co, si, so = pair
    for _ in range(32):
        client_done = server_done = False
        try:
            c.do_handshake()
            client_done = True
        except (ssl.SSLWantReadError, ssl.SSLWantWriteError):
            pass
        transfer(co, si)
        try:
            s.do_handshake()
            server_done = True
        except (ssl.SSLWantReadError, ssl.SSLWantWriteError):
            pass
        transfer(so, ci)
        if client_done and server_done:
            return
    raise AssertionError("handshake did not finish")


def dispose(pair):
    for obj in pair[:2]:
        # The adapter offers deterministic release of WIT resources.
        close = getattr(obj, "close", None)
        if close is not None:
            close()


def empty_write_handshake(cert_dir, tls12=False, trusted=True):
    pair = tls_pair(cert_dir, trusted=trusted,
                    maximum=ssl.TLSVersion.TLSv1_2 if tls12 else None)
    c, s, ci, co, si, so = pair
    try:
        try:
            c.write(b'')
        except ssl.SSLWantReadError:
            pass
        else:
            raise AssertionError('empty write skipped the handshake')
        assert co.pending > 0
        for _ in range(32):
            transfer(co, si)
            server_done = client_done = False
            try:
                assert s.write(b'') == 0
                server_done = True
            except (ssl.SSLWantReadError, ssl.SSLWantWriteError):
                pass
            transfer(so, ci)
            try:
                assert c.write(b'') == 0
                client_done = True
            except ssl.SSLCertVerificationError as error:
                assert not trusted and error.verify_code and error.verify_message
                assert error.reason == 'CERTIFICATE_VERIFY_FAILED'
                return 'empty-write-rejected'
            except (ssl.SSLWantReadError, ssl.SSLWantWriteError):
                pass
            if server_done and client_done:
                break
        else:
            raise AssertionError('empty writes did not complete the handshake')
        assert trusted and c.version() == s.version()
        transfer(co, si)
        assert c.write(b'') == 0 and co.pending == 0
        assert c.write(b'after-empty') == 11
        transfer(co, si)
        assert s.read(11) == b'after-empty'
        try:
            c.unwrap()
        except ssl.SSLWantReadError:
            pass
        else:
            raise AssertionError('shutdown did not wait for the peer')
        try:
            c.write(b'')
        except ssl.SSLError as error:
            assert error.reason == 'PROTOCOL_IS_SHUTDOWN', error
        else:
            raise AssertionError('empty write ignored TLS shutdown')
        return 'empty-write-handshake'
    finally:
        dispose(pair)


def exchange(cert_dir, tls12=False):
    pair = tls_pair(cert_dir, maximum=ssl.TLSVersion.TLSv1_2 if tls12 else None)
    c, s, ci, co, si, so = pair
    try:
        handshake(pair)
        assert c.selected_alpn_protocol() == "http/1.1"
        assert c.getpeercert(binary_form=True)
        assert c.cipher()[2] >= 128
        payload = b"\x00hello\xff" * 4096
        sent = c.write(payload)
        transfer(co, si)
        received = bytearray()
        while len(received) < sent:
            buffer = bytearray(1000)
            count = s.read(1000, buffer)
            received.extend(buffer[:count])
        assert bytes(received) == payload[:sent]
        s.write(b"response")
        transfer(so, ci)
        assert c.read() == b"response"
        return [c.version(), c.selected_alpn_protocol(), len(received)]
    finally:
        dispose(pair)


def rejection(cert_dir, name="localhost", trusted=True):
    pair = tls_pair(cert_dir, name=name, trusted=trusted)
    try:
        try:
            handshake(pair)
        except ssl.SSLCertVerificationError as error:
            assert error.verify_code != 0
            return "certificate-rejected"
        raise AssertionError("invalid certificate was accepted")
    finally:
        dispose(pair)


def symbolic_errors(cert_dir):
    # Manually constructed errors retain CPython's OSError storage and SSL str.
    for args in ((), ('message',), (1, 'message'), (1, None)):
        error = ssl.SSLError(*args)
        assert str(error) == (error.strerror if isinstance(error.strerror, str) else str(error.args))
        assert not hasattr(error, 'library') and not hasattr(error, 'reason')

    def check(call, expected_type, library, reason):
        try:
            call()
        except expected_type as error:
            assert error.library == library, (error.library, library, str(error))
            assert error.reason == reason, (error.reason, reason, str(error))
            assert str(error) == error.strerror
            assert isinstance(error.errno, int)
            return error
        raise AssertionError('operation did not raise expected SSL error')

    pair = tls_pair(cert_dir)
    try:
        check(pair[0].do_handshake, ssl.SSLWantReadError, None, None)
    finally:
        dispose(pair)
    for name, trusted in (('localhost', False), ('other.invalid', True)):
        pair = tls_pair(cert_dir, name=name, trusted=trusted)
        try:
            error = check(lambda: handshake(pair), ssl.SSLCertVerificationError,
                          'SSL', 'CERTIFICATE_VERIFY_FAILED')
            assert error.verify_code != 0
        finally:
            dispose(pair)
    pair = tls_pair(cert_dir)
    try:
        handshake(pair)
        pair[2].write_eof()
        check(pair[0].read, ssl.SSLEOFError, 'SSL', 'UNEXPECTED_EOF_WHILE_READING')
    finally:
        dispose(pair)
    pair = tls_pair(cert_dir)
    try:
        pair[4].write(b'GET / HTTP/1.0\r\n\r\n')
        check(pair[1].do_handshake, ssl.SSLError, 'SSL', 'HTTP_REQUEST')
    finally:
        dispose(pair)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    try:
        context.set_ciphers('NO_CIPHER_FOR_TEST')
    except ssl.SSLError as error:
        assert error.args == ('No cipher can be selected.',)
        assert not hasattr(error, 'library') and not hasattr(error, 'reason')
    else:
        raise AssertionError('invalid cipher accepted')
    check(lambda: context.load_cert_chain(cert_dir + '/weak-rsa.crt', cert_dir + '/weak-rsa.key'),
          ssl.SSLError, 'SSL', 'EE_KEY_TOO_SMALL')
    return 'symbolic-errors'


def unexpected_eof_options(cert_dir):
    for maximum in (ssl.TLSVersion.TLSv1_2, ssl.TLSVersion.TLSv1_3):
        for ignore in (False, True):
            client = ssl.create_default_context(cafile=cert_dir + '/server.crt')
            server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            server.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
            client.maximum_version = server.maximum_version = maximum
            if ignore:
                client.options |= ssl.OP_IGNORE_UNEXPECTED_EOF
            ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
            pair = (client.wrap_bio(ci, co, server_hostname='localhost'),
                    server.wrap_bio(si, so, server_side=True), ci, co, si, so)
            try:
                handshake(pair)
                pair[1].write(b'buffered before EOF')
                transfer(so, ci)
                ci.write_eof()
                assert pair[0].read() == b'buffered before EOF'
                if ignore:
                    assert pair[0].read() == b''
                else:
                    try:
                        pair[0].read()
                    except ssl.SSLEOFError as error:
                        assert error.library == 'SSL'
                        assert error.reason == 'UNEXPECTED_EOF_WHILE_READING'
                    else:
                        raise AssertionError('truncated TLS accepted')
            finally:
                dispose(pair)
    return 'unexpected-eof-options'


def verification_messages(cert_dir):
    for name, trusted, code, message in (
            ('localhost', False, 18, 'self-signed certificate'),
            ('other.invalid', True, 62, "Hostname mismatch, certificate is not valid for 'other.invalid'."),
            ('127.0.0.2', True, 64, "IP address mismatch, certificate is not valid for '127.0.0.2'.")):
        pair = tls_pair(cert_dir, name=name, trusted=trusted)
        try:
            try:
                handshake(pair)
            except ssl.SSLCertVerificationError as error:
                assert error.verify_code == code
                assert error.verify_message == message, (error.verify_message, message)
                assert error.library == 'SSL' and error.reason == 'CERTIFICATE_VERIFY_FAILED'
            else:
                raise AssertionError('certificate accepted')
        finally:
            dispose(pair)
    for revoked, code, message in ((False, 3, 'unable to get certificate CRL'),
                                   (True, 23, 'certificate revoked')):
        client = ssl.create_default_context(cafile=cert_dir + '/ca.crt')
        client.verify_flags |= ssl.VERIFY_CRL_CHECK_LEAF
        if revoked:
            client.load_verify_locations(cafile=cert_dir + '/revoked.crl')
        server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        server.load_cert_chain(cert_dir + '/leaf.crt', cert_dir + '/server.key')
        ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
        pair = (client.wrap_bio(ci, co, server_hostname='localhost'),
                server.wrap_bio(si, so, server_side=True), ci, co, si, so)
        try:
            try:
                handshake(pair)
            except ssl.SSLCertVerificationError as error:
                assert error.verify_code == code
                assert error.verify_message == message
                assert error.library == 'SSL' and error.reason == 'CERTIFICATE_VERIFY_FAILED'
            else:
                raise AssertionError('CRL failure accepted')
        finally:
            dispose(pair)
    return 'verification-messages'


def shutdown_and_eof(cert_dir):
    pair = tls_pair(cert_dir)
    c, s, ci, co, si, so = pair
    try:
        handshake(pair)
        try:
            c.unwrap()
        except ssl.SSLWantReadError:
            pass
        transfer(co, si)
        assert s.read() == b""
        s.unwrap()
        transfer(so, ci)
        c.unwrap()
    finally:
        dispose(pair)
    pair = tls_pair(cert_dir)
    try:
        handshake(pair)
        pair[2].write_eof()
        try:
            pair[0].read()
        except ssl.SSLEOFError:
            return "clean-close-and-eof"
        raise AssertionError("unexpected EOF was accepted")
    finally:
        dispose(pair)


def defaults_and_memory_bio():
    server_context = ssl.create_default_context(ssl.Purpose.CLIENT_AUTH)
    assert server_context.protocol == ssl.PROTOCOL_TLS_SERVER
    assert server_context.verify_mode == ssl.CERT_NONE
    assert not server_context.check_hostname
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    assert context.verify_mode == ssl.CERT_REQUIRED
    assert context.check_hostname
    try:
        context.verify_mode = ssl.CERT_NONE
    except ValueError:
        pass
    else:
        raise AssertionError("insecure context state was accepted")
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    context.check_hostname = True
    assert context.verify_mode == ssl.CERT_REQUIRED
    bio = ssl.MemoryBIO()
    assert bio.write(memoryview(b"abc")) == 3
    bio.write_eof()
    assert not bio.eof and bio.pending == 3
    assert bio.read(2) == b"ab"
    assert not bio.eof
    assert bio.read() == b"c" and bio.eof
    try:
        bio.write(b"x")
    except ssl.SSLError:
        return "defaults-and-bio"
    raise AssertionError("write after EOF was accepted")


def memory_bio_buffer_rules():
    # CPython 3.12 test_ssl.MemoryBIOTests.test_buffer_types plus boundary
    # and failure-order cases. Rejected input must not change pending data.
    bio = ssl.MemoryBIO()
    bio.write(b'kept')
    for data, kind in ((memoryview(b'abcdef')[::2], BufferError),
                       (memoryview(bytearray(b'abcdef'))[::-2], BufferError),
                       ('text', TypeError), (None, TypeError), (1, TypeError)):
        for eof in (False, True):
            candidate = ssl.MemoryBIO()
            candidate.write(b'kept')
            if eof:
                candidate.write_eof()
            try:
                candidate.write(data)
            except kind:
                pass
            else:
                raise AssertionError('invalid buffer accepted')
            assert candidate.pending == 4 and candidate.read() == b'kept'
            assert candidate.eof is eof
    for size in (1 << 31, -(1 << 31) - 1, 1 << 100):
        try:
            bio.read(size)
        except OverflowError:
            pass
        else:
            raise AssertionError('MemoryBIO read accepted an oversized C int')
        assert bio.pending == 4
    assert bio.read() == b'kept'
    from array import array
    for data in (b'abcd', bytearray(b'abcd'), array('I', [0x04030201]),
                 memoryview(b'abcd').cast('B', shape=[2, 2])):
        view = memoryview(data).cast('B')
        assert bio.write(data) == view.nbytes
        assert bio.read() == bytes(view)
    return 'memory-bio-buffer-rules'


def tls_buffer_rules(cert_dir, tls12=False):
    from array import array
    pair = tls_pair(cert_dir, maximum=ssl.TLSVersion.TLSv1_2 if tls12 else None)
    c, s, ci, co, si, so = pair
    try:
        assert c.read(0) == b'' and c.read(0, bytearray()) == 0
        for data in (memoryview(b'abcdef')[::2],
                     memoryview(bytearray(b'abcdef'))[::-2]):
            before = co.pending
            try:
                c.write(data)
            except BufferError:
                pass
            else:
                raise AssertionError('TLS write flattened a non-contiguous buffer')
            assert co.pending == before
        handshake(pair)
        for data in (b'abcd', bytearray(b'abcd'), array('I', [0x04030201]),
                     memoryview(b'abcd').cast('B', shape=[2, 2])):
            expected = bytes(memoryview(data).cast('B'))
            assert c.write(data) == len(expected)
            transfer(co, si)
            assert s.read(len(expected)) == expected
        # A bytes-returning zero read leaves even complete TLS records in the
        # input BIO. It must not enter OpenSSL or drain message callbacks.
        c.write(b'queued')
        transfer(co, si)
        pending = si.pending
        assert pending and s.read(0) == b'' and si.pending == pending
        assert s.read(6) == b'queued'
        for size in (0, -1, -5, 100):
            c.write(b'abcd')
            transfer(co, si)
            target = bytearray(b'xxxxxxxx')
            assert s.read(size, target) == 4
            assert target == b'abcdxxxx'
        for target in (bytearray(4), array('I', [0]),
                       memoryview(bytearray(4)).cast('B', shape=[2, 2])):
            c.write(b'abcd')
            transfer(co, si)
            assert s.read(4, target) == 4
            assert bytes(memoryview(target).cast('B')) == b'abcd'
        for target in (b'abcd', memoryview(bytearray(8))[::2],
                       memoryview(b'abcd'), None):
            try:
                s.read(-1, target)
            except (ValueError if target is None else TypeError):
                pass
            else:
                raise AssertionError('invalid TLS read destination accepted')
        for size in (sys.maxsize + 1, -sys.maxsize - 2):
            for target in (None, bytearray(4)):
                try:
                    s.read(size, target)
                except OverflowError:
                    pass
                else:
                    raise AssertionError('TLS read accepted an oversized ssize_t')
        # Validation failures did not consume plaintext or corrupt the session.
        c.write(b'after-errors')
        transfer(co, si)
        assert s.read(12) == b'after-errors'
        return 'tls-buffer-rules'
    finally:
        dispose(pair)


def hostname_argument_rules():
    # CPython 3.12 test_ssl.SSLErrorTests.test_bad_server_hostname, also
    # checked with hostname verification disabled and bytes A-labels.
    for checking in (False, True):
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        context.check_hostname = checking
        for name, kind in (('', ValueError), (b'', ValueError),
                           ('.example.org', ValueError), (b'.example.org', ValueError),
                           ('example.org\x00evil', TypeError), (b'example.org\x00evil', TypeError),
                           (b'\xff.invalid', UnicodeDecodeError)):
            incoming, outgoing = ssl.MemoryBIO(), ssl.MemoryBIO()
            try:
                context.wrap_bio(incoming, outgoing, server_hostname=name)
            except kind:
                pass
            else:
                raise AssertionError('invalid server hostname accepted')
            assert incoming.pending == outgoing.pending == 0
        for name, expected in (('localhost', 'localhost'), (b'localhost', 'localhost'),
                               ('täst.example', 'xn--tst-qla.example'),
                               ('127.0.0.1', '127.0.0.1'), ('::1', '::1')):
            obj = context.wrap_bio(ssl.MemoryBIO(), ssl.MemoryBIO(), server_hostname=name)
            try:
                assert obj.server_hostname == expected
            finally:
                close = getattr(obj, 'close', None)
                if close is not None:
                    close()
    return 'hostname-argument-rules'


def large_buffered_input(cert_dir):
    pair = tls_pair(cert_dir)
    c, s, ci, co, si, so = pair
    try:
        handshake(pair)
        payload = b"large-buffer" * 180000
        written = 0
        while written < len(payload):
            written += c.write(memoryview(payload)[written:])
        si.write(co.read())
        assert si.pending > 1024 * 1024
        received = bytearray()
        while len(received) < len(payload):
            received.extend(s.read(65536))
        assert bytes(received) == payload
        return len(received)
    finally:
        dispose(pair)


def certificate_metadata(cert_dir, verify=ssl.CERT_REQUIRED, mutual=False):
    import json
    client = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    client.check_hostname = False
    client.verify_mode = verify
    client.load_verify_locations(cafile=cert_dir + "/decoded.crt")
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server.load_cert_chain(cert_dir + "/decoded.crt", cert_dir + "/server.key")
    if mutual:
        client.load_cert_chain(cert_dir + "/decoded.crt", cert_dir + "/server.key")
        server.verify_mode = ssl.CERT_REQUIRED
        server.load_verify_locations(cafile=cert_dir + "/decoded.crt")
    ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
    pair = (client.wrap_bio(ci, co, server_hostname="localhost"),
            server.wrap_bio(si, so, server_side=True), ci, co, si, so)
    c, s = pair[:2]
    try:
        for binary in (False, True):
            try:
                c.getpeercert(binary)
            except ValueError:
                pass
            else:
                raise AssertionError("certificate available before handshake")
        handshake(pair)
        with open(cert_dir + "/decoded.crt") as file:
            expected_der = ssl.PEM_cert_to_DER_cert(file.read())
        assert c.getpeercert(True) == expected_der
        assert s.getpeercert(True) == (expected_der if mutual else None)
        with open(cert_dir + "/decoded.json") as file:
            expected = json.load(file)
        actual = c.getpeercert()
        if verify == ssl.CERT_NONE:
            assert actual == {}
        else:
            assert json.loads(json.dumps(actual)) == expected, (actual, expected)
            assert isinstance(actual["subject"], tuple)
            assert all(isinstance(rdn, tuple) and all(isinstance(attr, tuple) for attr in rdn)
                       for rdn in actual["subject"])
            assert len(actual["subject"][2]) == 2
            assert isinstance(actual["subjectAltName"][6][1], tuple)
            for name in ("subjectAltName", "OCSP", "caIssuers", "crlDistributionPoints"):
                assert isinstance(actual[name], tuple)
            actual["subject"] = ()
            assert c.getpeercert()["subject"]
        if mutual:
            assert json.loads(json.dumps(s.getpeercert())) == expected
        else:
            assert s.getpeercert() is None
        return "certificate-metadata"
    finally:
        dispose(pair)


def private_module_constants():
    import _ssl
    expected = {'PROTO_MINIMUM_SUPPORTED': -2, 'PROTO_MAXIMUM_SUPPORTED': -1,
                'PROTO_SSLv3': 768, 'PROTO_TLSv1': 769, 'PROTO_TLSv1_1': 770,
                'PROTO_TLSv1_2': 771, 'PROTO_TLSv1_3': 772}
    for name, value in expected.items():
        assert getattr(_ssl, name) == value and type(getattr(_ssl, name)) is int, name
    assert _ssl.HAS_TLS_UNIQUE is True
    for suffix in ('SSLv2', 'SSLv3', 'TLSv1', 'TLSv1_1', 'TLSv1_2', 'TLSv1_3'):
        name = 'HAS_' + suffix
        assert type(getattr(_ssl, name)) is bool and getattr(_ssl, name) is getattr(ssl, name), name
        name = 'OP_NO_' + suffix
        assert type(getattr(_ssl, name)) is int and getattr(_ssl, name) == getattr(ssl, name), name
    for suffix in ('SSLv23', 'TLSv1', 'TLSv1_1', 'TLSv1_2'):
        name = 'PROTOCOL_' + suffix
        assert type(getattr(_ssl, name)) is int and getattr(_ssl, name) == getattr(ssl, name), name
    default = _ssl._DEFAULT_CIPHERS
    assert type(default) is str and default
    if ssl.__name__ == 'tarantool_ssl':
        assert default == (
            '@SECLEVEL=2:ECDH+AESGCM:ECDH+CHACHA20:ECDH+AES:DHE+AES:!aNULL:!eNULL:!aDSS:!SHA1:!AESCCM')
    else:
        # CPython's --with-ssl-default-suites can select a distribution policy.
        # Its exported string must reproduce that build's default context.
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        ciphers = context.get_ciphers()
        context.set_ciphers(default)
        assert context.get_ciphers() == ciphers
    assert _ssl._DEFAULT_CIPHERS is default
    required = set(expected) | {'HAS_TLS_UNIQUE', '_DEFAULT_CIPHERS', '_OPENSSL_API_VERSION',
                               'PROTOCOL_TLSv1', 'OP_NO_SSLv3', 'HAS_TLSv1_3'}
    assert required <= set(dir(_ssl)), required - set(dir(_ssl))
    # Match CPython ssl.py's enum construction, which reads source.__dict__.
    import enum
    import sys
    import types
    module_name = 'ssl_private_constant_audit'
    audit = types.ModuleType(module_name)
    sys.modules[module_name] = audit
    try:
        for prefix, base in (('PROTOCOL_', enum.IntEnum), ('PROTO_', enum.IntEnum),
                             ('CERT_', enum.IntEnum), ('OP_', enum.IntFlag),
                             ('VERIFY_', enum.IntFlag), ('SSL_ERROR_', enum.IntEnum),
                             ('ALERT_DESCRIPTION_', enum.IntEnum)):
            names = [name for name in dir(_ssl) if name.startswith(prefix)]
            assert names and set(names) <= vars(_ssl).keys(), prefix
            cls = base._convert_('Audit', module_name, lambda name: name.startswith(prefix), source=_ssl)
            assert set(cls.__members__) == set(names), (prefix, cls.__members__)
            assert all(int(cls[name]) == getattr(_ssl, name) for name in names), prefix
    finally:
        sys.modules.pop(module_name)
    exported = {}
    exec('from _ssl import *', exported)
    for name in ('PROTOCOL_TLS_CLIENT', 'OP_NO_TLSv1_2', 'CERT_REQUIRED', 'VERIFY_X509_STRICT',
                 'SSL_ERROR_WANT_READ', 'ALERT_DESCRIPTION_INTERNAL_ERROR', 'HAS_TLSv1_3',
                 'PROTO_TLSv1_2', 'MemoryBIO', 'SSLError', 'Certificate', 'txt2obj'):
        assert exported[name] == getattr(_ssl, name), name
    assert 'os' not in exported and 'operator' not in exported
    try:
        _ssl.MISSING_TLSv1_CONSTANT
    except AttributeError:
        pass
    else:
        raise AssertionError('unknown private constant accepted')
    if ssl.__name__ == 'tarantool_ssl':
        assert default == ssl._host.default_ciphers()
        original_default = ssl._host.default_ciphers
        original_backend = ssl._host.backend
        original_protocols = ssl._host.protocol_capabilities
        calls = []
        def cipher_string():
            calls.append(True)
            if len(calls) == 1:
                raise RuntimeError('injected unavailable default cipher import')
            return 'test cipher policy'
        def forbidden():
            raise AssertionError('dir/private cipher string queried backend capabilities')
        try:
            del _ssl._DEFAULT_CIPHERS
            ssl._host.default_ciphers = cipher_string
            ssl._host.backend = ssl._host.protocol_capabilities = forbidden
            assert required <= set(dir(_ssl)) and not calls
            try:
                _ssl._DEFAULT_CIPHERS
            except RuntimeError:
                assert '_DEFAULT_CIPHERS' not in vars(_ssl)
            else:
                raise AssertionError('unavailable cipher import was frozen/cached')
            assert _ssl._DEFAULT_CIPHERS == 'test cipher policy'
            assert _ssl._DEFAULT_CIPHERS is _ssl._DEFAULT_CIPHERS and len(calls) == 2
        finally:
            ssl._host.default_ciphers = original_default
            ssl._host.backend = original_backend
            ssl._host.protocol_capabilities = original_protocols
            _ssl._DEFAULT_CIPHERS = default
    return 'private-module-constants'


def private_certificate_helpers(cert_dir):
    import _ssl
    import json
    import os
    api = _ssl._OPENSSL_API_VERSION
    assert type(api) is tuple and len(api) == 5
    assert all(type(part) is int for part in api)
    assert api[0] in (1, 3) and all(0 <= part <= 255 for part in api[1:4])
    assert 0 <= api[4] <= 15 and _ssl._OPENSSL_API_VERSION is api
    if ssl.__name__ == 'tarantool_ssl':
        number = ssl._host.compiled_api_version()
        assert api == ((number >> 28) & 15, (number >> 20) & 255,
                       (number >> 12) & 255, (number >> 4) & 255, number & 15)
        # Use deliberately different header/library versions: querying the
        # runtime backend here would silently misreport the compiled API.
        runtime_version = ssl.OPENSSL_VERSION_INFO
        original_compiled = ssl._host.compiled_api_version
        original_backend = ssl._host.backend
        calls = []
        def compiled():
            calls.append(True)
            if len(calls) == 1:
                raise RuntimeError('injected unavailable host import')
            return 0x1010117f  # 1.1.1w release headers.
        def forbidden_backend():
            raise AssertionError('compiled API queried the runtime library')
        try:
            del _ssl._OPENSSL_API_VERSION
            ssl._host.compiled_api_version = compiled
            ssl._host.backend = forbidden_backend
            try:
                _ssl._OPENSSL_API_VERSION
            except RuntimeError:
                assert '_OPENSSL_API_VERSION' not in vars(_ssl)
            else:
                raise AssertionError('unavailable host import was frozen/cached')
            assert _ssl._OPENSSL_API_VERSION == (1, 1, 1, 23, 15)
            assert _ssl._OPENSSL_API_VERSION is _ssl._OPENSSL_API_VERSION
            assert len(calls) == 2 and ssl.OPENSSL_VERSION_INFO is runtime_version
        finally:
            ssl._host.compiled_api_version = original_compiled
            ssl._host.backend = original_backend
            _ssl._OPENSSL_API_VERSION = api
    with open(cert_dir + '/decoded.json') as file:
        expected = json.load(file)
    decoded = _ssl._test_decode_cert(cert_dir + '/decoded.crt')
    assert json.loads(json.dumps(decoded)) == expected
    assert _ssl._test_decode_cert(os.fsencode(cert_dir + '/decoded.crt')) == decoded
    class Path:
        def __fspath__(self):
            return cert_dir + '/decoded.crt'
    assert _ssl._test_decode_cert(Path()) == decoded
    for path, error_type, message in (
            (cert_dir + '/absent.crt', ssl.SSLError, "Can't open file"),
            (cert_dir + '/server.key', ssl.SSLError, 'Error decoding PEM-encoded file'),
            ('bad\x00path', ValueError, None), (1, TypeError, None)):
        try:
            _ssl._test_decode_cert(path)
        except error_type as error:
            if message is not None:
                assert error.args == (message,), error.args
        else:
            raise AssertionError('invalid certificate path accepted')
    defaults = _ssl.get_default_verify_paths()
    assert isinstance(defaults, tuple) and len(defaults) == 4
    assert defaults == tuple(ssl.get_default_verify_paths()[2:])
    previous = os.environ.get('SSL_CERT_FILE')
    try:
        os.environ['SSL_CERT_FILE'] = '/missing-guest-cert.pem'
        assert _ssl.get_default_verify_paths() == defaults
    finally:
        if previous is None:
            os.environ.pop('SSL_CERT_FILE', None)
        else:
            os.environ['SSL_CERT_FILE'] = previous
    expected_oid = (129, 'serverAuth', 'TLS Web Server Authentication', '1.3.6.1.5.5.7.3.1')
    assert _ssl.txt2obj(expected_oid[-1]) == expected_oid
    assert _ssl.txt2obj('serverAuth', True) == expected_oid
    assert _ssl.nid2obj(129) == expected_oid
    try:
        _ssl.nid2obj(nid=129)
    except TypeError:
        pass
    else:
        raise AssertionError('nid keyword accepted')
    class InvalidBool:
        def __bool__(self):
            raise AssertionError('name converted before validating txt')
    for txt, error_type in ((None, TypeError), ('a\x00b', ValueError), ('\ud800', UnicodeEncodeError)):
        try:
            _ssl.txt2obj(txt, InvalidBool())
        except error_type:
            pass
        else:
            raise AssertionError('invalid text accepted')
    for call, args in ((_ssl.txt2obj, ('serverAuth',)), (_ssl.nid2obj, (0,))):
        try:
            call(*args)
        except ValueError:
            pass
        else:
            raise AssertionError('invalid OID accepted')
    return 'private-certificate-helpers'


def default_trust_overrides(cert_dir, temp_dir):
    import os
    names = ('SSL_CERT_FILE', 'SSL_CERT_DIR')
    saved = {name: os.environ.get(name) for name in names}
    try:
        for name in names:
            os.environ.pop(name, None)
        baseline = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        baseline.load_default_certs()
        defaults = baseline.cert_store_stats()
        for cached in (False, True):
            for file_value, dir_value, expected in (
                (cert_dir + '/ca.crt', None, {'x509': 1, 'x509_ca': 1, 'crl': 0}),
                (cert_dir + '/ca.crt', temp_dir, {'x509': 1, 'x509_ca': 1, 'crl': 0}),
                ('', '', {'x509': 0, 'x509_ca': 0, 'crl': 0}),
                (temp_dir + '/missing', temp_dir + '/missing', {'x509': 0, 'x509_ca': 0, 'crl': 0}),
                (None, temp_dir, defaults),
                (None, None, defaults),
            ):
                for name, value in zip(names, (file_value, dir_value)):
                    if value is None:
                        os.environ.pop(name, None)
                    else:
                        os.environ[name] = value
                ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
                if cached:
                    ctx.session_stats()  # Instantiate before adding trust.
                ctx.load_default_certs()
                assert ctx.cert_store_stats() == expected, (cached, file_value, dir_value, ctx.cert_store_stats())
                assert len(ctx.get_ca_certs()) == expected['x509_ca']
                assert len(ctx.get_ca_certs(binary_form=True)) == expected['x509_ca']
                # Later overrides do not remove anchors already loaded.
                os.environ['SSL_CERT_FILE'] = os.environ['SSL_CERT_DIR'] = ''
                ctx.load_default_certs()
                assert ctx.cert_store_stats() == expected
        return 'default-trust-overrides'
    finally:
        for name, value in saved.items():
            if value is None:
                os.environ.pop(name, None)
            else:
                os.environ[name] = value


def private_certificate_cleanup():
    # Fault injection is specific to the Python facade; native CPython uses
    # C allocations. Keep retained traceback frames alive during every check.
    if ssl.__name__ != 'tarantool_ssl':
        return 'native-certificate-cleanup-not-injected'
    import tarantool__ssl as module
    import gc
    class Handle:
        def __init__(self):
            self.closed = 0
        def __exit__(self, *args):
            self.closed += 1
            assert self.closed == 1, 'double resource drop'
    class Connection:
        def __init__(self, handles):
            self.handles = handles
        def certificate_chain(self, verified):
            return self.handles
    original = module.Certificate
    tracebacks = []
    try:
        for fail_at in (0, 1, 2):
            handles = [Handle() for _ in range(3)]
            class Factory:
                @staticmethod
                def _create(handle):
                    if handle is handles[fail_at]:
                        raise MemoryError('create failure')
                    return original._create(handle)
            module.Certificate = Factory
            try:
                module._chain(Connection(handles), False)
            except MemoryError as error:
                tracebacks.append(error.__traceback__)
            else:
                raise AssertionError('injected creation failure lost')
            assert [handle.closed for handle in handles] == [1, 1, 1]
            gc.collect()
            assert [handle.closed for handle in handles] == [1, 1, 1]
        module.Certificate = original
        for fail_at in (0, 1, 2):
            handles = [Handle() for _ in range(3)]
            class FailingList(list):
                def append(self, value):
                    if len(self) == fail_at:
                        raise MemoryError('append failure')
                    super().append(value)
            module.list = FailingList
            try:
                module._chain(Connection(handles), False)
            except MemoryError as error:
                tracebacks.append(error.__traceback__)
            else:
                raise AssertionError('injected append failure lost')
            assert [handle.closed for handle in handles] == [1, 1, 1]
            gc.collect()
            assert [handle.closed for handle in handles] == [1, 1, 1]
    finally:
        module.Certificate = original
        if 'list' in vars(module):
            del module.list
    tracebacks.clear()
    gc.collect()
    assert [handle.closed for handle in handles] == [1, 1, 1]
    return 'private-certificate-cleanup'


def private_certificate_chains(cert_dir, mutual=False, verified=True):
    import _ssl
    client = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    client.check_hostname = False
    client.verify_mode = ssl.CERT_REQUIRED if verified else ssl.CERT_NONE
    client.load_verify_locations(cafile=cert_dir + '/ca.crt')
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    # The server transmits only its leaf; verification adds the trusted root.
    server.load_cert_chain(cert_dir + '/leaf.crt', cert_dir + '/server.key')
    if mutual:
        client.load_cert_chain(cert_dir + '/client-leaf.crt', cert_dir + '/server.key')
        server.verify_mode = ssl.CERT_REQUIRED
        server.load_verify_locations(cafile=cert_dir + '/ca.crt')
    ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
    pair = (client.wrap_bio(ci, co, server_hostname='localhost'),
            server.wrap_bio(si, so, server_side=True), ci, co, si, so)
    c, s = pair[:2]
    try:
        for peer in (c, s):
            assert peer._sslobj.get_verified_chain() is None
            assert peer._sslobj.get_unverified_chain() is None
        handshake(pair)
        wire = c._sslobj.get_unverified_chain()
        chain = c._sslobj.get_verified_chain()
        assert len(wire) == (2 if mutual else 1) and len(chain) == 2
        with open(cert_dir + '/leaf.crt') as file:
            der = ssl.PEM_cert_to_DER_cert(file.read())
        with open(cert_dir + '/ca.crt') as file:
            ca_der = ssl.PEM_cert_to_DER_cert(file.read())
        assert wire[0].public_bytes(_ssl.ENCODING_DER) == der
        assert [cert.public_bytes(_ssl.ENCODING_DER) for cert in chain] == [der, ca_der]
        assert type(wire[0]) is _ssl.Certificate
        assert wire[0] is not chain[0] and wire[0] == chain[0]
        assert wire[0] != chain[1] and wire[0] != object()
        assert hash(wire[0]) == hash(chain[0]) and len(set(wire + chain)) == 2
        assert repr(wire[0]) == "<_ssl.Certificate 'CN=localhost'>", repr(wire[0])
        assert ssl.PEM_cert_to_DER_cert(wire[0].public_bytes()) == der
        aux = wire[0].public_bytes(0x101)
        assert aux.startswith('-----BEGIN TRUSTED CERTIFICATE-----')
        assert ssl.PEM_cert_to_DER_cert(aux.replace('TRUSTED CERTIFICATE', 'CERTIFICATE')) == der
        info = wire[0].get_info()
        assert info == _ssl._test_decode_cert(cert_dir + '/leaf.crt')
        info['subject'] = ()
        assert wire[0].get_info()['subject']
        for format, error in ((0, ValueError), (3, ValueError), (4, ValueError), ('pem', TypeError), (1 << 33, OverflowError)):
            try:
                wire[0].public_bytes(format)
            except error:
                pass
            else:
                raise AssertionError('invalid format accepted')
        try:
            _ssl.Certificate()
        except TypeError:
            pass
        else:
            raise AssertionError('public Certificate constructor accepted')
        try:
            class Invalid(_ssl.Certificate):
                pass
        except TypeError:
            pass
        else:
            raise AssertionError('Certificate subclass accepted')
        if mutual:
            server_wire = s._sslobj.get_unverified_chain()
            server_chain = s._sslobj.get_verified_chain()
            assert len(server_wire) == 2 and len(server_chain) == 2
            with open(cert_dir + '/client-leaf.crt') as file:
                client_der = ssl.PEM_cert_to_DER_cert(file.read())
            assert server_wire[0].public_bytes(2) == client_der
            assert server_wire[0] == server_chain[0]
        else:
            assert s._sslobj.get_unverified_chain() is None
            assert s._sslobj.get_verified_chain() is None
        dispose(pair)
        # X509 resources outlive their originating SSL connection and context.
        assert wire[0].public_bytes(2) == der
        assert wire[0].get_info()['subject'] and wire[0] == chain[0]
        assert repr(wire[0]) == "<_ssl.Certificate 'CN=localhost'>"
        return 'private-certificate-chains'
    finally:
        dispose(pair)


def private_bio_arguments():
    import _ssl
    import gc
    context = _ssl._SSLContext(_ssl.PROTOCOL_TLS_CLIENT)
    for attribute in ('protocol', 'minimum_version', 'maximum_version', 'verify_mode',
                      'verify_flags', 'options'):
        assert type(getattr(context, attribute)) is int, attribute
    assert type(_ssl.CERT_NONE) is int and type(_ssl.PROTOCOL_TLS_CLIENT) is int
    for name, error_type in ((b'localhost', TypeError), ('é.example', UnicodeEncodeError),
                             ('', ValueError), ('.example', ValueError), ('a\x00b', TypeError)):
        try:
            context._wrap_bio(_ssl.MemoryBIO(), _ssl.MemoryBIO(), False, name)
        except error_type:
            pass
        else:
            raise AssertionError('invalid private hostname accepted')
    # This differs from the public ssl.SSLContext.wrap_bio contract.
    obj = context._wrap_bio(_ssl.MemoryBIO(), _ssl.MemoryBIO(), False)
    assert obj.server_hostname is None and obj.owner is None
    assert obj.context is context and obj.server_side is False
    assert obj.version() is None and obj.cipher() is None and obj.pending() == 0
    assert obj.read(0) == b''
    for call in (lambda: obj.read(0, None), lambda: obj.read(size=0),
                 lambda: obj.write(b=b''), lambda: obj.getpeercert(der=True)):
        try:
            call()
        except TypeError:
            pass
        else:
            raise AssertionError('invalid private arguments accepted')
    class Owner:
        pass
    owner = Owner()
    obj.owner = owner
    assert obj.owner is owner
    del owner
    gc.collect()
    assert obj.owner is None
    for value in (None, 1):
        try:
            obj.owner = value
        except TypeError:
            pass
        else:
            raise AssertionError('owner does not require weak references')
    try:
        _ssl._SSLSocket()
    except TypeError:
        pass
    else:
        raise AssertionError('public private-socket constructor accepted')
    try:
        context._wrap_bio(_ssl.MemoryBIO(), _ssl.MemoryBIO(), True)
    except ssl.SSLError:
        pass
    else:
        raise AssertionError('wrong protocol role accepted')
    return 'private-bio-arguments'


def private_hostname_flags(cert_dir):
    import _ssl
    client = _ssl._SSLContext(_ssl.PROTOCOL_TLS_CLIENT)
    assert client._host_flags == 4 and type(client._host_flags) is int
    for name, value in (('ALWAYS_CHECK_SUBJECT', 1), ('NO_WILDCARDS', 2),
                        ('NO_PARTIAL_WILDCARDS', 4), ('MULTI_LABEL_WILDCARDS', 8),
                        ('SINGLE_LABEL_SUBDOMAINS', 16), ('NEVER_CHECK_SUBJECT', 32)):
        assert getattr(_ssl, 'HOSTFLAG_' + name) == value
    for value in (0, 4, 36, -1, 1 << 40):
        client._host_flags = value
        assert client._host_flags == value & 0xffffffff
    try:
        client._host_flags = 4.0
    except TypeError:
        pass
    else:
        raise AssertionError('floating point host flags accepted')
    client._host_flags = 4
    client.load_verify_locations(cafile=cert_dir + '/common-name.crt')
    server = _ssl._SSLContext(_ssl.PROTOCOL_TLS_SERVER)
    server.load_cert_chain(cert_dir + '/common-name.crt', cert_dir + '/server.key')
    def make_pair():
        ci, co, si, so = [_ssl.MemoryBIO() for _ in range(4)]
        return (client._wrap_bio(ci, co, False, 'localhost'),
                server._wrap_bio(si, so, True), ci, co, si, so)
    previous = make_pair()
    client._host_flags = 36
    # Changing another policy field must preserve the raw hostname flag bits.
    client.verify_flags ^= _ssl.VERIFY_X509_TRUSTED_FIRST
    assert client._host_flags == 36
    current = make_pair()
    handshake(previous)
    try:
        handshake(current)
    except ssl.SSLCertVerificationError as error:
        assert error.verify_code == 62, error
    else:
        raise AssertionError('NEVER_CHECK_SUBJECT accepted a CN-only certificate')
    client._host_flags = 4
    handshake(make_pair())
    return 'private-hostname-flags'


def private_binary_alpn(cert_dir, tls12=False):
    import _ssl
    for invalid in (b'\x00', b'\x03xx', b'\x02h2\x00'):
        context = _ssl._SSLContext(_ssl.PROTOCOL_TLS_CLIENT)
        try:
            context._set_alpn_protocols(invalid)
        except MemoryError:
            pass
        else:
            raise AssertionError('malformed binary ALPN accepted')
    context = _ssl._SSLContext(_ssl.PROTOCOL_TLS_CLIENT)
    for call, error in ((lambda: context._set_alpn_protocols('h2'), TypeError),
                        (lambda: context._set_alpn_protocols(protos=b'\x02h2'), TypeError),
                        (lambda: context._set_alpn_protocols(memoryview(b'abcd')[::2]), BufferError)):
        try:
            call()
        except error:
            pass
        else:
            raise AssertionError('invalid ALPN argument accepted')
    for name in (b'h2', b'\x00', b'\xc3\xa9', b'\xff', b'x' * 255, b''):
        client = _ssl._SSLContext(_ssl.PROTOCOL_TLS_CLIENT)
        client.load_verify_locations(cafile=cert_dir + '/server.crt')
        server = _ssl._SSLContext(_ssl.PROTOCOL_TLS_SERVER)
        server.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
        if tls12:
            client.maximum_version = server.maximum_version = ssl.TLSVersion.TLSv1_2
        wire = bytes([len(name)]) + name if name else b''
        client._set_alpn_protocols(memoryview(wire))
        # Server defaults remain live; the client offer is copied at SSL_new.
        server._set_alpn_protocols(b'\x05other')
        ci, co, si, so = [_ssl.MemoryBIO() for _ in range(4)]
        c = client._wrap_bio(ci, co, False, 'localhost')
        s = server._wrap_bio(si, so, True)
        assert c.selected_alpn_protocol() is None
        client._set_alpn_protocols(b'\x05other')
        server._set_alpn_protocols(bytearray(wire))
        handshake((c, s, ci, co, si, so))
        assert c.cipher() and c.version() and c.getpeercert()['subject']
        for peer in (c, s):
            if name == b'\xff':
                try:
                    peer.selected_alpn_protocol()
                except UnicodeDecodeError as error:
                    assert error.object == name and error.encoding == 'utf-8'
                else:
                    raise AssertionError('non-ASCII ALPN was silently decoded')
            else:
                assert peer.selected_alpn_protocol() == (name.decode('utf-8') if name else None)
        c.write(b'binary ALPN')
        transfer(co, si)
        assert s.read(11) == b'binary ALPN'
    if ssl.__name__ == 'tarantool_ssl':
        client._host_flags = 0x18
        client.hostname_checks_common_name = False
        assert client._host_flags == 0x38
        client.hostname_checks_common_name = True
        assert client._host_flags == 0x18
        client._set_alpn_protocols(b'\x03old')
        try:
            client._set_alpn_protocols(b'\x01x' * (512 * 1024 + 1))
        except ValueError:
            pass
        else:
            raise AssertionError('ALPN bound bypassed')
        assert client._private_alpn_wire == b'\x03old'
        client.set_alpn_protocols(['h2'])
        assert client._private_alpn_wire is None
        server.set_alpn_protocols(['h2'])
        ci, co, si, so = [_ssl.MemoryBIO() for _ in range(4)]
        c = client._wrap_bio(ci, co, False, 'localhost')
        s = server._wrap_bio(si, so, True)
        handshake((c, s, ci, co, si, so))
        assert c.selected_alpn_protocol() == s.selected_alpn_protocol() == 'h2'
    return 'private-binary-alpn'


def private_bio_exchange(cert_dir, tls12=False, owner_supplied=False):
    import _ssl
    client = _ssl._SSLContext(_ssl.PROTOCOL_TLS_CLIENT)
    client.load_verify_locations(cafile=cert_dir + '/server.crt')
    server = _ssl._SSLContext(_ssl.PROTOCOL_TLS_SERVER)
    server.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    if tls12:
        client.maximum_version = server.maximum_version = ssl.TLSVersion.TLSv1_2
    seen = []
    def sni(peer, name, context):
        seen.append((peer, name, context))
    server.sni_callback = sni
    messages = []
    def message(peer, direction, version, content, kind, data):
        assert type(version) is int and type(content) is int and type(kind) is int
        assert isinstance(data, bytes)
        messages.append(peer)
    server._msg_callback = message
    class Owner:
        pass
    owner = Owner() if owner_supplied else None
    ci, co, si, so = [_ssl.MemoryBIO() for _ in range(4)]
    c = client._wrap_bio(ci, co, False, 'localhost')
    s = server._wrap_bio(si, so, True, owner=owner)
    pair = c, s, ci, co, si, so
    handshake(pair)
    assert type(c) is _ssl._SSLSocket and type(s) is _ssl._SSLSocket
    assert seen == [(owner if owner_supplied else s, 'localhost', server)], seen
    assert messages and all(peer is (owner if owner_supplied else s) for peer in messages)
    assert c.version() == ('TLSv1.2' if tls12 else 'TLSv1.3')
    assert c.context is client and s.context is server
    assert c.server_hostname == 'localhost' and s.server_hostname is None
    assert c.compression() is None and c.cipher() and s.shared_ciphers()
    assert c.get_channel_binding() == s.get_channel_binding()
    assert c.getpeercert()['subject'] and c.get_verified_chain()[0].public_bytes(2) == c.getpeercert(True)
    assert c.write(memoryview(b'private-bio')) == 11
    transfer(co, si)
    buffer = bytearray(11)
    assert s.read(11, buffer) == 11 and buffer == b'private-bio'
    assert s.write(b'reply') == 5
    transfer(so, ci)
    assert c.read(5) == b'reply'
    assert not c.session_reused and c.session is not None
    try:
        c.shutdown()
    except ssl.SSLWantReadError:
        pass
    else:
        raise AssertionError('shutdown completed without peer acknowledgement')
    transfer(co, si)
    assert s.read(1) == b''
    assert s.shutdown() is None
    transfer(so, ci)
    assert c.shutdown() is None
    return 'private-bio-exchange'


def private_bio_dead_owner(cert_dir):
    import _ssl
    import gc
    client = _ssl._SSLContext(_ssl.PROTOCOL_TLS_CLIENT)
    client.load_verify_locations(cafile=cert_dir + '/server.crt')
    server = _ssl._SSLContext(_ssl.PROTOCOL_TLS_SERVER)
    server.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    called = []
    server.sni_callback = lambda *args: called.append(args)
    class Owner:
        pass
    owner = Owner()
    ci, co, si, so = [_ssl.MemoryBIO() for _ in range(4)]
    c = client._wrap_bio(ci, co, False, 'localhost')
    s = server._wrap_bio(si, so, True, owner=owner)
    del owner
    gc.collect()
    assert s.owner is None
    try:
        handshake((c, s, ci, co, si, so))
    except ssl.SSLError:
        pass
    else:
        raise AssertionError('collected SNI owner accepted')
    assert called == []
    assert so.pending > 0, 'fatal alert was not queued'
    return 'private-bio-dead-owner'


def certificate_helpers(cert_dir):
    with open(cert_dir + "/decoded.crt") as file:
        pem = file.read()
    der = ssl.PEM_cert_to_DER_cert(pem)
    assert ssl.DER_cert_to_PEM_cert(der) == pem
    assert ssl.DER_cert_to_PEM_cert(memoryview(der)) == pem
    assert ssl.PEM_cert_to_DER_cert(pem + " \n") == der
    empty = "-----BEGIN CERTIFICATE-----\n-----END CERTIFICATE-----\n"
    assert ssl.DER_cert_to_PEM_cert(b"") == empty
    assert ssl.PEM_cert_to_DER_cert(empty) == b""
    for invalid in (" " + pem, pem.replace("END CERTIFICATE", "END KEY")):
        try:
            ssl.PEM_cert_to_DER_cert(invalid)
        except ValueError:
            pass
        else:
            raise AssertionError("invalid PEM delimiters accepted")
    assert ssl.cert_time_to_seconds("Jan  1 00:00:00 1970 GMT") == 0
    assert ssl.cert_time_to_seconds("jAn  1 00:00:00 1970 GMT") == 0
    assert ssl.cert_time_to_seconds("Dec 31 23:59:60 1999 GMT") == 946684800
    for invalid in ("Foo  1 00:00:00 1970 GMT", "Jan  1 00:00:00 1970 UTC"):
        try:
            ssl.cert_time_to_seconds(invalid)
        except ValueError:
            pass
        else:
            raise AssertionError("invalid certificate time accepted")
    return "certificate-helpers"


def credential_formats(cert_dir):
    with open(cert_dir + "/server.crt") as file:
        pem = file.read()
    der = ssl.PEM_cert_to_DER_cert(pem)
    for cadata in (pem, der, der + der, bytearray(der), memoryview(der),
                   memoryview(der).cast("B", shape=[1, len(der)])):
        client = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        client.load_verify_locations(cadata=cadata)
        for password in ("fixture-password", b"fixture-password", bytearray(b"fixture-password"),
                         lambda: "fixture-password"):
            server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            server.load_cert_chain(cert_dir + "/server.crt", cert_dir + "/server-encrypted.key", password)
            ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
            pair = (client.wrap_bio(ci, co, server_hostname="localhost"),
                    server.wrap_bio(si, so, server_side=True), ci, co, si, so)
            try:
                handshake(pair)
                assert pair[0].getpeercert(True) == der
            finally:
                dispose(pair)
    return "der-and-encrypted-key"


def credential_load_errors(cert_dir):
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    def unused_password():
        raise AssertionError("password callback used for unencrypted key")
    context.load_cert_chain(cert_dir + "/server.crt", cert_dir + "/server.key", unused_password)
    for data, expected in (("", ValueError), (b"", ValueError), ("not PEM", ssl.SSLError),
                           (b"not DER", ssl.SSLError), ("non-ascii-\u044f", TypeError),
                           (memoryview(b"notder")[::2], BufferError), (42, TypeError)):
        try:
            context.load_verify_locations(cadata=data)
        except expected:
            pass
        else:
            raise AssertionError("invalid CA accepted at load time")
    for password, expected in (("wrong", ssl.SSLError), (42, TypeError),
                               (lambda: object(), TypeError)):
        try:
            context.load_cert_chain(cert_dir + "/server.crt", cert_dir + "/server-encrypted.key", password)
        except expected:
            pass
        else:
            raise AssertionError("invalid key password accepted at load time")
    error = ValueError("callback exception")
    def failing_password():
        raise error
    try:
        context.load_cert_chain(cert_dir + "/server.crt", cert_dir + "/server-encrypted.key", failing_password)
    except ValueError as caught:
        assert caught is error
    else:
        raise AssertionError("callback exception was lost")
    # CPython may prompt for an omitted password. The guest must never ask
    # the host terminal, so this adapter-specific rejection is not run there.
    if hasattr(ssl, "install"):
        try:
            context.load_cert_chain(cert_dir + "/server.crt", cert_dir + "/server-encrypted.key")
        except ssl.SSLError:
            pass
        else:
            raise AssertionError("encrypted key without a password accepted")
    return "credential-load-errors"


def policy_settings(cert_dir):
    import json
    fixed = ssl.SSLContext(ssl.PROTOCOL_TLSv1_2)
    assert fixed.minimum_version == ssl.TLSVersion.MINIMUM_SUPPORTED
    assert fixed.maximum_version == ssl.TLSVersion.MAXIMUM_SUPPORTED
    for name in ("minimum_version", "maximum_version"):
        try:
            setattr(fixed, name, ssl.TLSVersion.TLSv1_3)
        except ValueError:
            pass
        else:
            raise AssertionError("fixed protocol accepted a version bound")
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    assert isinstance(context.options, ssl.Options)
    assert isinstance(context.verify_flags, ssl.VerifyFlags)
    assert isinstance(context.verify_mode, ssl.VerifyMode)
    assert context.options & ssl.OP_NO_COMPRESSION
    assert context.verify_flags & ssl.VERIFY_X509_TRUSTED_FIRST
    assert context.hostname_checks_common_name
    context.options |= ssl.OP_NO_TICKET
    assert context.options & ssl.OP_NO_TICKET
    context.options &= ~ssl.OP_NO_TICKET
    assert not context.options & ssl.OP_NO_TICKET
    context.set_ciphers("ECDHE-RSA-AES128-GCM-SHA256")
    with open(cert_dir + "/ciphers.json") as file:
        assert context.get_ciphers() == json.load(file)
    assert context.security_level == 2
    context.set_ciphers("ECDHE-RSA-AES128-GCM-SHA256:@SECLEVEL=1")
    assert context.security_level == 1
    try:
        context.options = -1
    except OverflowError:
        pass
    else:
        raise AssertionError("negative options mask accepted")
    context.verify_flags = -1
    assert int(context.verify_flags) == 0xffffffffffffffff
    context.verify_flags = 1 << 64
    assert int(context.verify_flags) == 0
    context.verify_flags = ssl.VERIFY_X509_TRUSTED_FIRST
    for name in ("minimum_version", "maximum_version"):
        try:
            setattr(context, name, 42)
        except ValueError:
            pass
        else:
            raise AssertionError("invalid TLS bound accepted")
    context.minimum_version = ssl.TLSVersion.MAXIMUM_SUPPORTED
    assert context.minimum_version == ssl.TLSVersion.TLSv1_3
    context.maximum_version = ssl.TLSVersion.MINIMUM_SUPPORTED
    assert context.maximum_version == ssl.TLSVersion.TLSv1
    try:
        context.set_ciphers("not-a-cipher")
    except ssl.SSLError:
        pass
    else:
        raise AssertionError("invalid cipher list accepted")
    return "policy-settings"


def policy_exchange(cert_dir, mode):
    certfile = cert_dir + ("/common-name.crt" if mode.startswith("common-name") else "/server.crt")
    client = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    client.load_verify_locations(cafile=cert_dir + "/ca.crt" if mode == "crl-required" else certfile)
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server.load_cert_chain(cert_dir + "/leaf.crt" if mode == "crl-required" else certfile, cert_dir + "/server.key")
    if mode in ("cipher", "legacy-cipher-metadata"):
        client.maximum_version = server.maximum_version = ssl.TLSVersion.TLSv1_2
        cipher = "ECDHE-RSA-AES128-GCM-SHA256" if mode == "cipher" else "ECDHE-RSA-AES128-SHA:@SECLEVEL=0"
        client.set_ciphers(cipher)
        server.set_ciphers(cipher)
    elif mode == "disable-tls13":
        client.options |= ssl.OP_NO_TLSv1_3
    elif mode == "disable-tls12":
        client.maximum_version = ssl.TLSVersion.TLSv1_2
        client.options |= ssl.OP_NO_TLSv1_2
    elif mode == "common-name-disabled":
        client.hostname_checks_common_name = False
    elif mode == "crl-required":
        client.verify_flags |= ssl.VERIFY_CRL_CHECK_LEAF
    elif mode == "generic-tls":
        client = ssl.SSLContext(ssl.PROTOCOL_TLS)
        client.load_verify_locations(cafile=certfile)
        client.verify_mode = ssl.CERT_REQUIRED
        client.check_hostname = True
        server = ssl.SSLContext(ssl.PROTOCOL_TLS)
        server.load_cert_chain(certfile, cert_dir + "/server.key")
    elif mode == "fixed-tls12":
        client = ssl.SSLContext(ssl.PROTOCOL_TLSv1_2)
        client.load_verify_locations(cafile=certfile)
        client.verify_mode = ssl.CERT_REQUIRED
        client.check_hostname = True
        server = ssl.SSLContext(ssl.PROTOCOL_TLSv1_2)
        server.load_cert_chain(certfile, cert_dir + "/server.key")
    ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
    pair = (client.wrap_bio(ci, co, server_hostname="localhost"),
            server.wrap_bio(si, so, server_side=True), ci, co, si, so)
    try:
        if mode in ("disable-tls12", "common-name-disabled", "crl-required"):
            try:
                handshake(pair)
            except ssl.SSLError:
                return "policy-rejected"
            raise AssertionError("OpenSSL policy was ignored")
        handshake(pair)
        if mode in ("cipher", "disable-tls13", "fixed-tls12"):
            assert pair[0].version() == "TLSv1.2"
        if mode == "cipher":
            assert pair[0].cipher()[0] == "ECDHE-RSA-AES128-GCM-SHA256"
        if mode == "legacy-cipher-metadata":
            assert pair[0].version() == "TLSv1.2"
            assert pair[0].cipher() == ("ECDHE-RSA-AES128-SHA", "TLSv1.0", 128)
        return "policy-applied"
    finally:
        dispose(pair)


def store_introspection(cert_dir):
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    assert context.cert_store_stats() == {"x509": 0, "x509_ca": 0, "crl": 0}
    assert context.get_ca_certs() == []
    context.load_verify_locations(cafile=cert_dir + "/empty.crl")
    assert context.cert_store_stats() == {"x509": 0, "x509_ca": 0, "crl": 1}
    context.load_verify_locations(cafile=cert_dir + "/server.crt")
    context.load_verify_locations(cafile=cert_dir + "/server.crt")
    context.load_verify_locations(cafile=cert_dir + "/leaf.crt")
    assert context.cert_store_stats() == {"x509": 2, "x509_ca": 1, "crl": 1}
    with open(cert_dir + "/server.crt") as file:
        der = ssl.PEM_cert_to_DER_cert(file.read())
    assert context.get_ca_certs(True) == [der]
    decoded = context.get_ca_certs()
    assert len(decoded) == 1 and decoded[0]["serialNumber"]
    with open(cert_dir + "/server.crt") as file:
        cert = file.read()
    with open(cert_dir + "/empty.crl") as file:
        crl = file.read()
    data_context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    data_context.load_verify_locations(cadata=cert + crl)
    assert data_context.cert_store_stats() == {"x509": 1, "x509_ca": 1, "crl": 0}
    try:
        data_context.load_verify_locations(cadata=crl)
    except ssl.SSLError:
        pass
    else:
        raise AssertionError("cadata loaded a CRL without a certificate")
    assert data_context.get_ca_certs() == decoded
    return "store-introspection"


def store_exchange(cert_dir, mode):
    client = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    client.load_verify_locations(cafile=cert_dir + ("/rejected-ca.pem" if mode == "aux-rejected" else "/ca.crt"))
    client.verify_flags |= ssl.VERIFY_CRL_CHECK_LEAF
    if mode != "missing":
        client.load_verify_locations(cafile=cert_dir + ("/revoked.crl" if mode == "revoked" else "/empty.crl"))
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server.load_cert_chain(cert_dir + "/leaf.crt", cert_dir + "/server.key")
    ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
    pair = (client.wrap_bio(ci, co, server_hostname="localhost"),
            server.wrap_bio(si, so, server_side=True), ci, co, si, so)
    try:
        if mode != "valid":
            try:
                handshake(pair)
            except ssl.SSLCertVerificationError as error:
                assert error.verify_code == {"missing": 3, "revoked": 23, "aux-rejected": 28}[mode]
                return "store-rejected"
            raise AssertionError("trust store verification was ignored")
        handshake(pair)
        assert pair[0].write(b"crl-checked") == 11
        transfer(co, si)
        assert pair[1].read(11) == b"crl-checked"
        return "store-accepted"
    finally:
        dispose(pair)


def store_pem_whitespace(cert_dir):
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    context.load_verify_locations(cafile=cert_dir + "/padded-store.pem")
    assert context.cert_store_stats() == {"x509": 1, "x509_ca": 1, "crl": 1}
    assert len(context.get_ca_certs(True)) == 1
    return "store-pem-whitespace"


def connection_metadata(cert_dir, tls12=False):
    pair = tls_pair(cert_dir, maximum=ssl.TLSVersion.TLSv1_2 if tls12 else None)
    client, server = pair[:2]
    client.context.set_ciphers("ECDHE-RSA-AES128-GCM-SHA256:ECDHE-RSA-AES256-GCM-SHA384")
    server.context.set_ciphers("ECDHE-RSA-AES256-GCM-SHA384:ECDHE-RSA-AES128-GCM-SHA256")
    # Context changes affect new wraps. Recreate the pair with these settings.
    dispose(pair)
    ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
    pair = (client.context.wrap_bio(ci, co, server_hostname="localhost"),
            server.context.wrap_bio(si, so, server_side=True), ci, co, si, so)
    client, server = pair[:2]
    try:
        for peer in (client, server):
            assert peer.get_channel_binding() is None and peer.shared_ciphers() is None
            for kind, error_type in (("tls-exporter", ValueError), (b"tls-unique", TypeError), (None, TypeError)):
                try:
                    peer.get_channel_binding(kind)
                except error_type:
                    pass
                else:
                    raise AssertionError("unsupported channel binding type accepted")
        handshake(pair)
        binding = client.get_channel_binding()
        assert isinstance(binding, bytes) and len(binding) == (12 if tls12 else 48)
        assert server.get_channel_binding() == binding
        assert client.shared_ciphers() is None
        offered = {cipher["name"] for cipher in client.context.get_ciphers()
                   if not tls12 or cipher["protocol"] != "TLSv1.3"}
        expected = [(cipher["name"], cipher["protocol"], cipher["strength_bits"])
                    for cipher in server.context.get_ciphers() if cipher["name"] in offered]
        assert server.shared_ciphers() == expected
        assert server.cipher() in expected
        client.write(b"channel-bound")
        transfer(co, si)
        assert server.read() == b"channel-bound"
        assert client.get_channel_binding() == server.get_channel_binding() == binding
        return "connection-metadata"
    finally:
        dispose(pair)


def session_resumption(cert_dir, tls12=False, use_setter=False):
    import time
    client_context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    client_context.load_verify_locations(cafile=cert_dir + "/server.crt")
    server_context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server_context.load_cert_chain(cert_dir + "/server.crt", cert_dir + "/server.key")
    if tls12:
        client_context.maximum_version = server_context.maximum_version = ssl.TLSVersion.TLSv1_2
    expected_stats = {"number", "connect", "connect_good", "connect_renegotiate", "accept",
                      "accept_good", "accept_renegotiate", "hits", "misses", "timeouts", "cache_full"}
    assert set(client_context.session_stats()) == expected_stats
    assert all(value == 0 for value in client_context.session_stats().values())
    saved = None
    for attempt in range(2):
        ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
        client = client_context.wrap_bio(ci, co, server_hostname="localhost",
                                        session=None if use_setter else saved)
        server = server_context.wrap_bio(si, so, server_side=True)
        pair = client, server, ci, co, si, so
        try:
            assert not client.session_reused
            if saved is None or use_setter:
                assert client.session is None
            else:
                assert client.session == saved
            if saved is not None and use_setter:
                client.session = saved
            handshake(pair)
            assert client.session_reused == server.session_reused == bool(attempt)
            # Process TLS 1.3 tickets together with server application data.
            server.write(b"ticket-data")
            transfer(so, ci)
            assert client.read() == b"ticket-data"
            session = client.session
            assert isinstance(session, ssl.SSLSession)
            assert session.id and session.has_ticket
            assert abs(session.time - time.time()) < 60
            assert session.timeout > 0 and session.ticket_lifetime_hint > 0
            assert session == client.session and session != object()
            try:
                hash(session)
            except TypeError:
                pass
            else:
                raise AssertionError("SSLSession must be unhashable")
            try:
                session.timeout = 1
            except AttributeError:
                pass
            else:
                raise AssertionError("SSLSession metadata must be read-only")
            try:
                client.session = session
            except ValueError:
                pass
            else:
                raise AssertionError("set_session allowed after handshake")
            saved = session
            assert client.get_channel_binding() == server.get_channel_binding()
            assert client_context.session_stats()["connect_good"] == attempt + 1
            assert server_context.session_stats()["accept_good"] == attempt + 1
            # Complete TLS shutdown so OpenSSL retains a resumable session.
            client_done = server_done = False
            for _ in range(16):
                if not client_done:
                    try:
                        client.unwrap()
                        client_done = True
                    except (ssl.SSLWantReadError, ssl.SSLWantWriteError):
                        pass
                transfer(co, si)
                if not server_done:
                    try:
                        server.unwrap()
                        server_done = True
                    except (ssl.SSLWantReadError, ssl.SSLWantWriteError):
                        pass
                transfer(so, ci)
                if client_done and server_done:
                    break
            assert client_done and server_done
        finally:
            dispose(pair)
    assert client_context.session_stats()["hits"] == server_context.session_stats()["hits"] == 1
    assert saved.id and saved.has_ticket
    return "session-resumption"


def session_errors(cert_dir):
    pair = tls_pair(cert_dir)
    client, server = pair[:2]
    try:
        handshake(pair)
        session = client.session
        assert session is not None
        for peer in (client, server):
            try:
                peer.session = None
            except TypeError:
                pass
            else:
                raise AssertionError("invalid session type accepted")
        other = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        try:
            other.wrap_bio(ssl.MemoryBIO(), ssl.MemoryBIO(), server_hostname="localhost", session=session)
        except ValueError:
            pass
        else:
            raise AssertionError("foreign context session accepted")
        try:
            server.context.wrap_bio(ssl.MemoryBIO(), ssl.MemoryBIO(), server_side=True, session=session)
        except ValueError:
            pass
        else:
            raise AssertionError("server session accepted")
        try:
            ssl.SSLSession()
        except TypeError:
            pass
        else:
            raise AssertionError("SSLSession has no public constructor")
        return "session-errors"
    finally:
        dispose(pair)



def key_exchange(cert_dir, mode, dhfile=None):
    if dhfile is None:
        dhfile = cert_dir + "/dh2048.pem"
    client = ssl.create_default_context(cafile=cert_dir + '/server.crt')
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    if mode.startswith('dh'):
        client.maximum_version = server.maximum_version = ssl.TLSVersion.TLSv1_2
        client.set_ciphers('DHE-RSA-AES256-GCM-SHA384')
        server.set_ciphers('DHE-RSA-AES256-GCM-SHA384')
        if mode != 'dh-missing':
            server.load_dh_params(dhfile)
            for invalid_file in (cert_dir + "/dh1024.pem", cert_dir + "/server.crt"):
                try:
                    server.load_dh_params(invalid_file)
                except ssl.SSLError:
                    pass
                else:
                    raise AssertionError("invalid DH parameters replaced the valid group")
    else:
        if mode.endswith('12'):
            client.maximum_version = server.maximum_version = ssl.TLSVersion.TLSv1_2
            client.set_ciphers('ECDHE-RSA-AES256-GCM-SHA384')
            server.set_ciphers('ECDHE-RSA-AES256-GCM-SHA384')
        client.set_ecdh_curve('prime256v1')
        server.set_ecdh_curve('secp384r1' if mode.startswith('mismatch') else 'prime256v1')
    ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
    pair = (client.wrap_bio(ci, co, server_hostname='localhost'),
            server.wrap_bio(si, so, server_side=True), ci, co, si, so)
    try:
        if mode.startswith('mismatch') or mode == 'dh-missing':
            try:
                handshake(pair)
            except ssl.SSLError:
                return 'key-exchange-rejected'
            raise AssertionError('incompatible key exchange succeeded')
        handshake(pair)
        c, s = pair[:2]
        assert c.cipher()[0] == s.cipher()[0]
        c.write(b'key exchange')
        si.write(co.read())
        assert s.read() == b'key exchange'
        return 'key-exchange-ok'
    finally:
        dispose(pair)


def public_pem_password_rejection(cert_dir):
    from wit_world.imports import ssl_bio as host
    from wit_world import Err
    def encrypted(label):
        return (f"-----BEGIN {label}-----\nProc-Type: 4,ENCRYPTED\n"
                "DEK-Info: AES-256-CBC,00000000000000000000000000000000\n\n"
                f"AAAAAAAAAAAAAAAAAAAAAA==\n-----END {label}-----\n").encode()
    with open(cert_dir + "/server.key", "rb") as file:
        key = file.read()
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    certificate = encrypted("CERTIFICATE")
    calls = (
        (host.prepare_ca, host.CertificateInput_Pem(certificate)),
        (host.prepare_credentials, certificate, key, None),
        (host.prepare_store_pem, encrypted("TRUSTED CERTIFICATE")),
        (host.prepare_store_pem, encrypted("X509 CRL")),
        (host.prepare_key_exchange, host.KeyExchange(dh_pem=encrypted("DH PARAMETERS"), curve=None), context._policy()),
    )
    for operation, *arguments in calls:
        try:
            operation(*arguments)
        except Err as error:
            assert error.value.kind == host.ErrorKind.FAILURE
        else:
            raise AssertionError("encrypted public PEM accepted")
    return "public-pem-password-rejected"


def live_alpn(cert_dir, mode, tls12=False):
    client = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    if tls12:
        client.maximum_version = server.maximum_version = ssl.TLSVersion.TLSv1_2
    client.set_alpn_protocols(['h2', 'http/1.1'])
    server.set_alpn_protocols([] if mode == 'enable' else ['h2', 'http/1.1'])
    def make_pair():
        ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
        return (client.wrap_bio(ci, co, server_hostname='localhost'),
                server.wrap_bio(si, so, server_side=True), ci, co, si, so)
    previous = make_pair()
    current = final = None
    try:
        if mode == 'client':
            client.set_alpn_protocols(['http/1.1'])
        elif mode == 'disable':
            server.set_alpn_protocols([])
        elif mode == 'no-overlap':
            server.set_alpn_protocols(['other'])
        else:
            server.set_alpn_protocols(['http/1.1'])
        # Trust added after changing ALPN must reach the already created SSL.
        client.load_verify_locations(cafile=cert_dir + '/server.crt')
        handshake(previous)
        expected_old = 'h2' if mode == 'client' else (
            None if mode in ('disable', 'no-overlap') else 'http/1.1')
        assert previous[0].selected_alpn_protocol() == previous[1].selected_alpn_protocol() == expected_old
        current = make_pair()
        handshake(current)
        expected_new = None if mode in ('disable', 'no-overlap') else 'http/1.1'
        assert current[0].selected_alpn_protocol() == current[1].selected_alpn_protocol() == expected_new
        for value, error in (([b'h2'], TypeError), ([1], TypeError), (['é'], UnicodeEncodeError),
                             ([''], ssl.SSLError), (['x' * 256], ssl.SSLError)):
            try:
                server.set_alpn_protocols(value)
            except error:
                pass
            else:
                raise AssertionError('invalid ALPN accepted')
        final = make_pair()
        handshake(final)
        assert final[0].selected_alpn_protocol() == expected_new
        assert client.session_stats()['connect_good'] == server.session_stats()['accept_good'] == 3
        assert client.cert_store_stats()['x509'] == 1
        previous[0].write(b'old ALPN connection')
        transfer(previous[3], previous[4])
        assert previous[1].read() == b'old ALPN connection'
    finally:
        for pair in (previous, current, final):
            if pair is not None:
                dispose(pair)
    return 'live-alpn'


def live_key_exchange(cert_dir, mode):
    client = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    client.maximum_version = server.maximum_version = ssl.TLSVersion.TLSv1_2
    server.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    if mode == 'dh':
        client.set_ciphers('DHE-RSA-AES256-GCM-SHA384')
        server.set_ciphers('DHE-RSA-AES256-GCM-SHA384')
    else:
        client.set_ciphers('ECDHE-RSA-AES256-GCM-SHA384')
        server.set_ciphers('ECDHE-RSA-AES256-GCM-SHA384')
        client.set_ecdh_curve('prime256v1')
        server.set_ecdh_curve('prime256v1')
    def make_pair():
        ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
        return (client.wrap_bio(ci, co, server_hostname='localhost'),
                server.wrap_bio(si, so, server_side=True), ci, co, si, so)
    previous = make_pair()
    current = final = None
    try:
        if mode == 'dh':
            server.load_dh_params(cert_dir + '/dh2048.pem')
        else:
            server.set_ecdh_curve('secp384r1')
        client.load_verify_locations(cafile=cert_dir + '/server.crt')
        try:
            handshake(previous)
        except ssl.SSLError:
            assert mode == 'dh'
        else:
            assert mode == 'curve'
        current = make_pair()
        try:
            handshake(current)
        except ssl.SSLError:
            assert mode == 'curve'
        else:
            assert mode == 'dh'
        assert server.session_stats()['accept_good'] == client.session_stats()['connect_good'] == 1
        # Failed replacement must keep the last valid parameter.
        try:
            if mode == 'dh':
                server.load_dh_params(cert_dir + '/dh1024.pem')
            else:
                server.set_ecdh_curve('RSA')
        except ssl.SSLError:
            pass
        else:
            raise AssertionError('invalid replacement accepted')
        if mode == 'curve':
            server.set_ecdh_curve('prime256v1')
        final = make_pair()
        handshake(final)
        assert server.session_stats()['accept_good'] == client.session_stats()['connect_good'] == 2
        final[0].write(b'retained key exchange')
        transfer(final[3], final[4])
        assert final[1].read() == b'retained key exchange'
        assert client.cert_store_stats()['x509'] == 1
    finally:
        for pair in (previous, current, final):
            if pair is not None:
                dispose(pair)
    # Changing a curve must not revalidate an unrelated, formerly allowed DH
    # parameter after the security level was increased, as in CPython.
    server.set_ciphers('DEFAULT:@SECLEVEL=1')
    server.load_dh_params(cert_dir + '/dh1024.pem')
    server.set_ciphers('DEFAULT:@SECLEVEL=2')
    server.set_ecdh_curve('prime256v1')
    assert server.security_level == 2 and server.session_stats()['accept_good'] == 2
    return 'live-key-exchange'


def dh_security_history(cert_dir, cached):
    client = ssl.create_default_context(cafile=cert_dir + '/server.crt')
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    client.maximum_version = server.maximum_version = ssl.TLSVersion.TLSv1_2
    if cached:
        server.session_stats()
    server.set_ciphers('DEFAULT:@SECLEVEL=1')
    server.load_dh_params(cert_dir + '/dh1024.pem')
    server.set_ciphers('DEFAULT:@SECLEVEL=2')
    server.set_ecdh_curve('prime256v1')
    assert server.security_level == 2
    assert server.session_stats()['accept_good'] == 0
    def make_pair():
        ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
        return (client.wrap_bio(ci, co, server_hostname='localhost'),
                server.wrap_bio(si, so, server_side=True), ci, co, si, so)
    client.set_ciphers('ECDHE-RSA-AES128-GCM-SHA256')
    pair = make_pair()
    try:
        handshake(pair)  # old DH parameters are irrelevant to ECDHE
        assert pair[0].cipher()[0] == 'ECDHE-RSA-AES128-GCM-SHA256'
    finally:
        dispose(pair)
    client.set_ciphers('DHE-RSA-AES128-GCM-SHA256')
    server.set_ciphers('DHE-RSA-AES128-GCM-SHA256')
    pair = make_pair()
    try:
        try:
            handshake(pair)
        except ssl.SSLError:
            pass
        else:
            raise AssertionError('historical DH1024 bypassed the current security level')
        assert server.security_level == 2
        assert server.session_stats()['accept_good'] == 1
    finally:
        dispose(pair)
    return 'dh-security-history'


def key_exchange_settings(cert_dir):
    import pathlib
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    for name in ('prime256v1', 'secp384r1', 'X25519', b'prime256v1', pathlib.Path('prime256v1')):
        assert context.set_ecdh_curve(name) is None
    for name, error in (('P-256', ValueError), ('unknown', ValueError),
                        ('prime256v1:secp384r1', ValueError), ('prime256v1\0x', ValueError),
                        ('RSA', ssl.SSLError), (None, TypeError), (1, TypeError)):
        try:
            context.set_ecdh_curve(name)
        except error:
            pass
        else:
            raise AssertionError('invalid curve accepted: ' + repr(name))
    assert context.load_dh_params(pathlib.Path(cert_dir + '/dh2048.pem')) is None
    for path, error in ((cert_dir + '/dh1024.pem', ssl.SSLError),
                        (cert_dir + '/server.crt', ssl.SSLError),
                        (cert_dir + '/missing-dh.pem', FileNotFoundError), (None, TypeError), (1, TypeError)):
        try:
            context.load_dh_params(path)
        except error:
            pass
        else:
            raise AssertionError('invalid DH parameters accepted')
    context.set_ciphers('DEFAULT:@SECLEVEL=0')
    assert context.load_dh_params(cert_dir + '/dh1024.pem') is None
    return 'key-exchange-settings'



def sni_exchange(cert_dir, mode='switch', tls12=False):
    client = ssl.create_default_context(cafile=cert_dir + '/ca.crt')
    original = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    original.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    replacement = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    replacement.load_cert_chain(cert_dir + '/leaf.crt', cert_dir + '/server.key')
    client.set_alpn_protocols(['h2', 'http/1.1'])
    original.set_alpn_protocols(['http/1.1'])
    replacement.set_alpn_protocols(['h2'])
    if mode == 'hrr':
        original.set_ecdh_curve('prime256v1')
        replacement.set_ecdh_curve('prime256v1')
        def second(obj, name, context):
            assert context is replacement and obj.context is replacement
            seen.append(name)
        replacement.sni_callback = second
    if tls12:
        client.maximum_version = original.maximum_version = replacement.maximum_version = ssl.TLSVersion.TLSv1_2
    hostname = 'localhost'
    if mode in ('idna', 'legacy'):
        hostname = 'täst.example'
        client.check_hostname = False
    elif mode == 'absent':
        hostname = None
        client.check_hostname = False
    elif mode == 'ip':
        hostname = '127.0.0.1'
    seen = []
    def choose(obj, name, context):
        assert isinstance(obj, ssl.SSLObject) and obj.server_side and context is original
        assert obj.context is original
        assert obj.version() is None
        seen.append(name)
        if mode == 'alert':
            return ssl.ALERT_DESCRIPTION_UNRECOGNIZED_NAME
        if mode == 'exception':
            raise RuntimeError('SNI callback fixture failure')
        if mode == 'bad-return':
            return 'not an alert number'
        obj.context = replacement
        assert obj.context is replacement
        try:
            obj.context = None
        except TypeError:
            pass
        else:
            raise AssertionError('invalid context accepted')
        if mode == 'switch-alert':
            return ssl.ALERT_DESCRIPTION_UNRECOGNIZED_NAME
    if mode in ('pre-both', 'pre-enable', 'pre-disable'):
        def unexpected(*args):
            raise AssertionError('old context callback invoked')
        if mode == 'pre-disable':
            def fallback(obj, name, context):
                assert context is original and obj.context is replacement
                seen.append(name)
            original.sni_callback = fallback
        elif mode != 'pre-enable':
            original.sni_callback = unexpected
        if mode != 'pre-disable':
            def selected(obj, name, context):
                assert context is replacement and obj.context is replacement
                seen.append(name)
            replacement.sni_callback = selected
    elif mode == 'legacy':
        original.set_servername_callback(choose)
    else:
        original.sni_callback = choose
    ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
    pair = (client.wrap_bio(ci, co, server_hostname=hostname), original.wrap_bio(si, so, server_side=True), ci, co, si, so)
    try:
        if mode.startswith('pre-'):
            pair[1].context = replacement
        if mode in ('alert', 'exception', 'bad-return', 'switch-alert'):
            import contextlib
            import io
            try:
                with contextlib.redirect_stderr(io.StringIO()):
                    handshake(pair)
            except ssl.SSLError:
                assert seen == ['localhost']
                return 'sni-rejected'
            raise AssertionError('callback alert ignored')
        handshake(pair)
        expected = 'xn--tst-qla.example' if mode in ('idna', 'legacy') else None if mode in ('absent', 'ip') else 'localhost'
        assert seen == [expected] * (2 if mode == 'hrr' and not tls12 else 1), seen
        c, s = pair[:2]
        assert s.context is replacement
        with open(cert_dir + '/leaf.crt') as file:
            assert c.getpeercert(True) == ssl.PEM_cert_to_DER_cert(file.read())
        assert c.selected_alpn_protocol() == s.selected_alpn_protocol() == 'h2'
        c.write(b'sni selection')
        si.write(co.read())
        assert s.read() == b'sni selection'
        return 'sni-exchange'
    finally:
        dispose(pair)


def module_constants_and_star_import():
    names = {}
    exec('from ssl import *', names)
    for name in ('SSLContext', 'SSLObject', 'SSLSocket', 'SSLSession', 'MemoryBIO',
                 'OPENSSL_VERSION', 'OPENSSL_VERSION_NUMBER', 'OPENSSL_VERSION_INFO',
                 'HAS_SSLv3', 'HAS_TLSv1', 'HAS_TLSv1_1', 'HAS_TLSv1_2', 'HAS_TLSv1_3',
                 'PEM_HEADER', 'PEM_FOOTER', 'create_connection', 'socket_error',
                 'SOCK_STREAM', 'SOL_SOCKET', 'SO_TYPE', 'OP_LEGACY_SERVER_CONNECT',
                 'OP_ENABLE_KTLS', 'OP_IGNORE_UNEXPECTED_EOF'):
        assert names[name] == getattr(ssl, name), name
    assert ssl.OP_LEGACY_SERVER_CONNECT == 4 and ssl.OP_ENABLE_KTLS == 8
    assert ssl.OP_IGNORE_UNEXPECTED_EOF == 128
    for name in ('OP_LEGACY_SERVER_CONNECT', 'OP_ENABLE_KTLS', 'OP_IGNORE_UNEXPECTED_EOF'):
        assert getattr(ssl.Options, name) == getattr(ssl, name)
    assert ssl.PEM_HEADER == '-----BEGIN CERTIFICATE-----'
    assert ssl.PEM_FOOTER == '-----END CERTIFICATE-----'
    import socket
    assert ssl.create_connection is socket.create_connection and ssl.socket_error is socket.error
    assert ssl.socket is socket.socket and names['socket'] is socket.socket
    return 'module-constants-and-star-import'


def policy_mask_normalization(cert_dir, cached):
    for count in (1, 2):
        client = ssl.create_default_context(cafile=cert_dir + '/ca.crt')
        server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        server.load_cert_chain(cert_dir + '/leaf.crt', cert_dir + '/server.key')
        if cached:
            client.session_stats()
        for flag in (0x100, 0x200, 0x400):
            client.verify_flags = 0
            client.verify_flags = flag
            assert client.verify_flags == flag | 0x80
            client.verify_flags = flag
            assert client.verify_flags == flag
        client.verify_flags = ssl.VERIFY_X509_TRUSTED_FIRST
        for _ in range(count):
            client.verify_flags = 0x100
        assert client.verify_flags == (0x180 if count == 1 else 0x100)
        ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
        pair = client.wrap_bio(ci, co, server_hostname='localhost'), server.wrap_bio(si, so, server_side=True), ci, co, si, so
        try:
            try:
                handshake(pair)
            except ssl.SSLCertVerificationError:
                assert count == 1
            else:
                assert count == 2
                assert pair[0].getpeercert()['subject']
        finally:
            dispose(pair)
    return 'policy-mask-normalization'


def raw_policy_masks(cert_dir, cached):
    import warnings
    client = ssl.create_default_context(cafile=cert_dir + '/server.crt')
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    if cached:
        client.session_stats()
    original_options, original_flags = client.options, client.verify_flags
    with warnings.catch_warnings():
        warnings.simplefilter('ignore', DeprecationWarning)
        for attribute, initial in (('options', original_options), ('verify_flags', original_flags)):
            for mask in (4, 8, 128, 1 << 63, (1 << 64) - 1, initial):
                setattr(client, attribute, mask)
                ci, co = ssl.MemoryBIO(), ssl.MemoryBIO()
                obj = client.wrap_bio(ci, co, server_hostname='localhost')
                try:
                    assert int(getattr(client, attribute)) == mask
                    assert client.security_level == 2
                finally:
                    close = getattr(obj, 'close', None)
                    if close is not None:
                        close()
            setattr(client, attribute, initial)
    # Removing raw flags must restore an ordinary verified handshake and
    # preserve the live context's trust/session state across later mutations.
    saved = None
    for attempt in range(2):
        if attempt:
            client.options |= ssl.OP_ENABLE_KTLS | ssl.OP_LEGACY_SERVER_CONNECT
            client.options = original_options
            client.verify_flags = (1 << 64) - 1
            client.verify_flags = original_flags
        ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
        pair = client.wrap_bio(ci, co, server_hostname='localhost', session=saved), server.wrap_bio(si, so, server_side=True), ci, co, si, so
        try:
            handshake(pair)
            assert pair[0].session_reused is bool(attempt)
            pair[1].write(b'raw masks ticket')
            transfer(so, ci)
            assert pair[0].read() == b'raw masks ticket'
            saved = pair[0].session
            assert saved.has_ticket
            assert client.session_stats()['connect_good'] == attempt + 1
            assert client.session_stats()['hits'] == attempt
            assert client.cert_store_stats()['x509'] == 1
        finally:
            dispose(pair)
    return 'raw-policy-masks'


def protocol_bounds(cert_dir, cached):
    import warnings
    with warnings.catch_warnings():
        warnings.simplefilter('ignore', DeprecationWarning)
        for protocol in (ssl.PROTOCOL_TLS, ssl.PROTOCOL_TLS_CLIENT, ssl.PROTOCOL_TLS_SERVER):
            for field in ('minimum_version', 'maximum_version'):
                for bound in ssl.TLSVersion:
                    ctx = ssl.SSLContext(protocol)
                    if cached:
                        ctx.session_stats()
                    setattr(ctx, field, bound)
                    expected = (ssl.TLSVersion.TLSv1_3 if field == 'minimum_version' and bound == ssl.TLSVersion.MAXIMUM_SUPPORTED
                                else ssl.TLSVersion.TLSv1 if field == 'maximum_version' and bound == ssl.TLSVersion.MINIMUM_SUPPORTED else bound)
                    assert getattr(ctx, field) == expected
                    assert bool(ctx.options & ssl.OP_NO_SSLv3) is not (field == 'minimum_version' and bound == ssl.TLSVersion.SSLv3)
                    assert ctx.security_level == 2
        for protocol in (ssl.PROTOCOL_TLSv1, ssl.PROTOCOL_TLSv1_1, ssl.PROTOCOL_TLSv1_2):
            ctx = ssl.SSLContext(protocol)
            assert ctx.minimum_version == ssl.TLSVersion.MINIMUM_SUPPORTED
            assert ctx.maximum_version == ssl.TLSVersion.MAXIMUM_SUPPORTED
            if cached:
                ctx.session_stats()
            for field in ('minimum_version', 'maximum_version'):
                try:
                    setattr(ctx, field, ssl.TLSVersion.TLSv1_3)
                except ValueError:
                    pass
                else:
                    raise AssertionError('fixed legacy protocol accepted version update')
                for value, error in (('TLSv1', TypeError), (None, TypeError), (1 << 31, OverflowError),
                                     (1 << 64, OverflowError), (42, ValueError)):
                    try:
                        setattr(ctx, field, value)
                    except error:
                        pass
                    else:
                        raise AssertionError('fixed protocol argument conversion differs')
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        for field in ('minimum_version', 'maximum_version'):
            before = getattr(ctx, field)
            for value, error in ((42, ValueError), (1 << 64, OverflowError), ('TLSv1', TypeError)):
                try:
                    setattr(ctx, field, value)
                except error:
                    pass
                else:
                    raise AssertionError('invalid version update accepted')
                assert getattr(ctx, field) == before
    assert ssl.HAS_TLSv1 and ssl.HAS_TLSv1_1 and ssl.HAS_TLSv1_2 and ssl.HAS_TLSv1_3
    assert not ssl.HAS_SSLv3
    return 'protocol-bounds'


def legacy_protocol_exchange(cert_dir, tls11, fixed, cached):
    import warnings
    with warnings.catch_warnings():
        warnings.simplefilter('ignore', DeprecationWarning)
        method = (ssl.PROTOCOL_TLSv1_1 if tls11 else ssl.PROTOCOL_TLSv1) if fixed else ssl.PROTOCOL_TLS
        client, server = ssl.SSLContext(method), ssl.SSLContext(method)
        client.verify_mode = ssl.CERT_REQUIRED
        client.check_hostname = True
        client.load_verify_locations(cafile=cert_dir + '/server.crt')
        server.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
        if cached:
            client.session_stats()
            server.session_stats()
        # Legacy handshakes need an explicit policy opt-in on modern OpenSSL.
        for ctx in (client, server):
            ctx.set_ciphers('DEFAULT:@SECLEVEL=0')
        def make_pair(session=None):
            ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
            return (client.wrap_bio(ci, co, server_hostname='localhost', session=session),
                    server.wrap_bio(si, so, server_side=True), ci, co, si, so)
        before = None
        current = resumed = None
        if not fixed:
            for ctx in (client, server):
                ctx.maximum_version = ssl.TLSVersion.TLSv1_2
            before = make_pair()
            for ctx in (client, server):
                ctx.minimum_version = ctx.maximum_version = ssl.TLSVersion.TLSv1_1 if tls11 else ssl.TLSVersion.TLSv1
        try:
            if before is not None:
                handshake(before)
                assert before[0].version() == 'TLSv1.2'
            current = make_pair()
            handshake(current)
            assert current[0].version() == ('TLSv1.1' if tls11 else 'TLSv1')
            assert current[0].getpeercert()['subject']
            current[1].write(b'legacy ticket')
            transfer(current[5], current[2])
            assert current[0].read() == b'legacy ticket'
            saved = current[0].session
            assert saved.has_ticket
            resumed = make_pair(saved)
            handshake(resumed)
            assert resumed[0].session_reused and resumed[1].session_reused
            assert client.session_stats()['connect_good'] == server.session_stats()['accept_good'] == (2 if fixed else 3)
            assert client.session_stats()['hits'] == server.session_stats()['hits'] == 1
            assert client.security_level == server.security_level == 0
        finally:
            for pair in (before, current, resumed):
                if pair is not None:
                    dispose(pair)
    return 'legacy-protocol-exchange'


def legacy_policy_rejection(cert_dir):
    import warnings
    with warnings.catch_warnings():
        warnings.simplefilter('ignore', DeprecationWarning)
        client = ssl.create_default_context(cafile=cert_dir + '/server.crt')
        server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        server.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
        for ctx in (client, server):
            ctx.minimum_version = ctx.maximum_version = ssl.TLSVersion.TLSv1_1
            ctx.set_ciphers('DEFAULT')
        ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
        pair = client.wrap_bio(ci, co, server_hostname='localhost'), server.wrap_bio(si, so, server_side=True), ci, co, si, so
        try:
            try:
                handshake(pair)
            except ssl.SSLError:
                pass
            else:
                raise AssertionError('legacy version bound silently weakened security')
            assert client.security_level == server.security_level == 2
        finally:
            dispose(pair)
    return 'legacy-policy-rejected'


def live_credentials(cert_dir, mode, tls12=False):
    client = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    if mode == 'client':
        client.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
        server.verify_mode = ssl.CERT_REQUIRED
    if tls12:
        client.maximum_version = server.maximum_version = ssl.TLSVersion.TLSv1_2
    def make_pair(session=None):
        ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
        return (client.wrap_bio(ci, co, server_hostname='localhost', session=session),
                server.wrap_bio(si, so, server_side=True), ci, co, si, so)
    previous = make_pair()
    current = resumed = None
    try:
        target = client if mode == 'client' else server
        replacement = 'client-leaf' if mode == 'client' else 'leaf'
        target.load_cert_chain(cert_dir + '/' + replacement + '.crt', cert_dir + '/server.key')
        with open(cert_dir + '/server.crt') as source:
            old_pem = source.read()
        with open(cert_dir + '/ca.crt') as source:
            trust = old_pem + source.read()
        for context in (client, server):
            context.load_verify_locations(cadata=trust)
        handshake(previous)
        observer = 1 if mode == 'client' else 0
        assert previous[observer].getpeercert(True) == ssl.PEM_cert_to_DER_cert(old_pem)
        previous[1].write(b'old credential ticket')
        transfer(previous[5], previous[2])
        assert previous[0].read() == b'old credential ticket'
        saved = previous[0].session
        assert saved.has_ticket
        current = make_pair()
        handshake(current)
        with open(cert_dir + '/' + replacement + '.crt') as source:
            assert current[observer].getpeercert(True) == ssl.PEM_cert_to_DER_cert(source.read())
        # A failed weak replacement must leave the last successful pair.
        try:
            target.load_cert_chain(cert_dir + '/weak-rsa.crt', cert_dir + '/weak-rsa.key')
        except ssl.SSLError:
            pass
        else:
            raise AssertionError('weak credential replacement accepted')
        resumed = make_pair(saved)
        handshake(resumed)
        assert resumed[0].session_reused and resumed[1].session_reused
        assert client.session_stats()['connect_good'] == server.session_stats()['accept_good'] == 3
        assert client.session_stats()['hits'] == server.session_stats()['hits'] == 1
        assert client.cert_store_stats()['x509'] == server.cert_store_stats()['x509'] == 2
        previous[0].write(b'old credentials remain valid')
        transfer(previous[3], previous[4])
        assert previous[1].read() == b'old credentials remain valid'
    finally:
        for pair in (previous, current, resumed):
            if pair is not None:
                dispose(pair)
    return 'live-credentials'


def credential_security_history(cert_dir, cached):
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    if cached:
        server.session_stats()
    calls = []
    def password():
        calls.append(True)
        return 'fixture-password'
    try:
        server.load_cert_chain(cert_dir + '/weak-rsa.crt', cert_dir + '/weak-rsa-encrypted.key', password)
    except ssl.SSLError:
        pass
    else:
        raise AssertionError('weak public certificate accepted')
    assert not calls
    try:
        server.load_cert_chain(cert_dir + '/weak-chain.pem', cert_dir + '/server-encrypted.key', password)
    except ssl.SSLError:
        pass
    else:
        raise AssertionError('weak chain certificate accepted')
    assert not calls
    server.set_ciphers('DEFAULT:@SECLEVEL=1')
    server.load_cert_chain(cert_dir + '/weak-rsa.crt', cert_dir + '/weak-rsa-encrypted.key', password)
    assert calls == [True]
    server.set_ciphers('DEFAULT:@SECLEVEL=2')
    assert server.security_level == 2 and server.session_stats()['accept_good'] == 0
    client = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    client.check_hostname = False
    client.verify_mode = ssl.CERT_NONE
    client.set_ciphers('DEFAULT:@SECLEVEL=1')
    ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
    pair = client.wrap_bio(ci, co, server_hostname='localhost'), server.wrap_bio(si, so, server_side=True), ci, co, si, so
    try:
        try:
            handshake(pair)
        except ssl.SSLError:
            pass
        else:
            raise AssertionError('historical load bypassed current handshake security')
        assert server.security_level == 2
    finally:
        dispose(pair)
    return 'credential-security-history'


def credential_algorithms(cert_dir, cached):
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server.maximum_version = ssl.TLSVersion.TLSv1_2
    if cached:
        server.session_stats()
    for name in ('server', 'ec'):
        server.load_cert_chain(cert_dir + '/' + name + '.crt', cert_dir + '/' + name + '.key')
    for number, (algorithm, name) in enumerate((('RSA', 'server'), ('ECDSA', 'ec')), 1):
        client = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        client.maximum_version = ssl.TLSVersion.TLSv1_2
        client.set_ciphers('ECDHE-' + algorithm + '-AES256-GCM-SHA384')
        client.load_verify_locations(cafile=cert_dir + '/' + name + '.crt')
        ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
        pair = client.wrap_bio(ci, co, server_hostname='localhost'), server.wrap_bio(si, so, server_side=True), ci, co, si, so
        try:
            handshake(pair)
            with open(cert_dir + '/' + name + '.crt') as source:
                assert pair[0].getpeercert(True) == ssl.PEM_cert_to_DER_cert(source.read())
            assert server.session_stats()['accept_good'] == number
        finally:
            dispose(pair)
    return 'credential-algorithms'


def generic_context_roles(cert_dir, fixed=False, pha=False):
    context = ssl.SSLContext(ssl.PROTOCOL_TLSv1_2 if fixed else ssl.PROTOCOL_TLS)
    context.verify_mode = ssl.CERT_REQUIRED
    context.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    context.post_handshake_auth = pha
    context.set_alpn_protocols(['h2', 'http/1.1'])
    seen = []
    def choose(peer, name, origin):
        # Native generic contexts also invoke their servername callback on a
        # client receiving an SNI acknowledgement. This adapter's bridge is
        # for server ClientHello callbacks; count the server selections here.
        if not peer.server_side:
            return
        assert peer.server_side and origin is context and name == 'localhost'
        seen.append(name)
    context.sni_callback = choose
    saved = None
    for attempt in range(2):
        ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
        pair = (context.wrap_bio(ci, co, server_hostname='localhost', session=None if pha else saved),
                context.wrap_bio(si, so, server_side=True), ci, co, si, so)
        try:
            if attempt == 0:
                # Both SSL objects must share the same live store despite
                # having opposite roles, and use the current server ALPN list.
                context.load_verify_locations(cafile=cert_dir + '/server.crt')
                context.set_alpn_protocols(['http/1.1'])
            handshake(pair)
            c, s = pair[:2]
            assert c.server_side is False and s.server_side is True
            assert c.context is s.context is context
            assert c.selected_alpn_protocol() == s.selected_alpn_protocol() == 'http/1.1'
            assert c.session_reused is (attempt > 0 and not pha)
            assert c.getpeercert()['subject']
            if pha:
                assert s.getpeercert() is None
                s.verify_client_post_handshake()
                s.write(b'role challenge')
                transfer(so, ci)
                assert c.read() == b'role challenge'
                c.write(b'role response')
                transfer(co, si)
                assert s.read() == b'role response'
            assert s.getpeercert()['subject']
            s.write(b'role ticket data')
            transfer(so, ci)
            assert c.read() == b'role ticket data'
            saved = c.session
            assert saved.has_ticket
            stats = context.session_stats()
            assert stats['accept_good'] == stats['connect_good'] == attempt + 1
            assert stats['hits'] == (0 if pha else attempt * 2)
            assert context.cert_store_stats()['x509'] == 1
            assert seen == ['localhost'] * (attempt + 1)
        finally:
            dispose(pair)
    return 'generic-context-roles'


def live_sni(cert_dir, mode, tls12=False):
    client = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    original = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    replacement = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    for context in (original, replacement):
        context.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    if tls12:
        client.maximum_version = original.maximum_version = replacement.maximum_version = ssl.TLSVersion.TLSv1_2
    seen = []
    def old(peer, name, context):
        assert name == 'localhost' and context is original
        seen.append('old')
    def new(peer, name, context):
        assert name == 'localhost'
        seen.append('new')
        if mode in ('self-disable', 'self-alert'):
            context.sni_callback = None
            if mode == 'self-alert':
                return ssl.ALERT_DESCRIPTION_UNRECOGNIZED_NAME
    if mode not in ('enable', 'fallback-enable'):
        original.sni_callback = old
    if mode == 'current-disable':
        replacement.sni_callback = new
    switched = mode.startswith(('fallback-', 'current-'))
    def make_pair(session=None):
        ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
        pair = (client.wrap_bio(ci, co, server_hostname='localhost', session=session),
                original.wrap_bio(si, so, server_side=True), ci, co, si, so)
        if switched:
            pair[1].context = replacement
        return pair
    previous = make_pair()
    current = None
    try:
        target = replacement if mode.startswith('current-') else original
        if mode == 'invalid':
            try:
                target.sni_callback = 123
            except TypeError:
                pass
            else:
                raise AssertionError('invalid SNI callback accepted')
            assert target.sni_callback is None
        else:
            target.sni_callback = None if mode in ('disable', 'fallback-disable', 'current-disable') else new
        # SNI changes retain both trust/counters and the context's ticket keys.
        client.load_verify_locations(cafile=cert_dir + '/server.crt')
        original.load_verify_locations(cafile=cert_dir + '/server.crt')
        if mode == 'self-alert':
            try:
                handshake(previous)
            except ssl.SSLError:
                assert seen == ['new']
                assert original.sni_callback is None
                return 'live-sni'
            raise AssertionError('SNI alert lost after callback disabled itself')
        handshake(previous)
        expected = [] if mode in ('disable', 'fallback-disable', 'invalid') else (
            ['old'] if mode == 'current-disable' else ['new'])
        assert seen == expected, (mode, seen)
        previous[1].write(b'SNI ticket data')
        transfer(previous[5], previous[2])
        assert previous[0].read() == b'SNI ticket data'
        saved = previous[0].session
        assert saved.has_ticket
        current = make_pair(saved)
        handshake(current)
        assert current[0].session_reused and current[1].session_reused
        assert seen == expected * (1 if mode == 'self-disable' else 2), (mode, seen)
        assert client.session_stats()['connect_good'] == 2 and client.session_stats()['hits'] == 1
        active = replacement if switched else original
        assert active.session_stats()['accept_good'] == 2
        assert original.cert_store_stats()['x509'] == 1
        previous[0].write(b'old SNI connection')
        transfer(previous[3], previous[4])
        assert previous[1].read() == b'old SNI connection'
    finally:
        for pair in (previous, current):
            if pair is not None:
                dispose(pair)
    return 'live-sni'


def sni_acknowledgement(cert_dir):
    for enabled, hostname in ((True, 'localhost'), (False, 'localhost'), (True, None)):
        pair = tls_pair(cert_dir, maximum=ssl.TLSVersion.TLSv1_2)
        c, s, ci, co, si, so = pair
        dispose(pair)
        client, server = c.context, s.context
        client.check_hostname = hostname is not None
        selected = []
        if enabled:
            def choose(peer, name, context):
                selected.append(name)
                # A replacement without a callback still acknowledges the
                # successfully handled ClientHello server name.
                replacement = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
                replacement.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
                replacement.maximum_version = ssl.TLSVersion.TLSv1_2
                peer.context = replacement
            server.sni_callback = choose
        ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
        pair = client.wrap_bio(ci, co, server_hostname=hostname), server.wrap_bio(si, so, server_side=True), ci, co, si, so
        c, s = pair[:2]
        flight = bytearray()
        try:
            for _ in range(32):
                done_c = done_s = False
                try:
                    c.do_handshake()
                    done_c = True
                except ssl.SSLWantReadError:
                    pass
                transfer(co, si)
                try:
                    s.do_handshake()
                    done_s = True
                except ssl.SSLWantReadError:
                    pass
                data = so.read()
                flight.extend(data)
                ci.write(data)
                if done_c and done_s:
                    break
            else:
                raise AssertionError('SNI acknowledgement handshake stalled')
            # TLS 1.2 ServerHello is plaintext: parse record/handshake lengths
            # and its extension vector instead of relying on metadata alone.
            handshakes = bytearray()
            offset = 0
            while offset + 5 <= len(flight):
                size = int.from_bytes(flight[offset + 3:offset + 5], 'big')
                if flight[offset] == 22:
                    handshakes.extend(flight[offset + 5:offset + 5 + size])
                offset += 5 + size
            assert handshakes[0] == 2
            size = int.from_bytes(handshakes[1:4], 'big')
            hello = handshakes[4:4 + size]
            offset = 35 + hello[34] + 3
            extensions = hello[offset + 2:offset + 2 + int.from_bytes(hello[offset:offset + 2], 'big')]
            types = []
            offset = 0
            while offset + 4 <= len(extensions):
                kind = int.from_bytes(extensions[offset:offset + 2], 'big')
                size = int.from_bytes(extensions[offset + 2:offset + 4], 'big')
                types.append(kind)
                if kind == 0:
                    assert size == 0
                offset += 4 + size
            assert (0 in types) == (enabled and hostname is not None), types
            assert selected == ([hostname] if enabled else [])
        finally:
            dispose(pair)
    return 'sni-acknowledgement'


def post_handshake_auth(cert_dir, mode='cert', tls12=False):
    client = ssl.create_default_context(cafile=cert_dir + '/server.crt')
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    server.load_verify_locations(cafile=cert_dir + '/server.crt')
    client.post_handshake_auth = mode != 'disabled'
    server.post_handshake_auth = True
    server.verify_mode = ssl.CERT_REQUIRED if mode == 'missing-required' else ssl.CERT_OPTIONAL
    if mode == 'no-verification':
        server.verify_mode = ssl.CERT_NONE
    if mode in ('cert', 'untrusted'):
        client.load_cert_chain(cert_dir + ('/leaf.crt' if mode == 'untrusted' else '/server.crt'), cert_dir + '/server.key')
    if tls12:
        client.maximum_version = server.maximum_version = ssl.TLSVersion.TLSv1_2
    ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
    pair = client.wrap_bio(ci, co, server_hostname='localhost'), server.wrap_bio(si, so, server_side=True), ci, co, si, so
    c, s = pair[:2]
    try:
        handshake(pair)
        if not tls12:
            assert s.getpeercert() is None
        try:
            c.verify_client_post_handshake()
        except ssl.SSLError:
            pass
        else:
            raise AssertionError('client requested client authentication')
        queued = so.pending
        try:
            s.verify_client_post_handshake()
        except ssl.SSLError:
            assert mode in ('disabled', 'no-verification') or tls12
            return 'post-auth-rejected'
        assert not tls12 and mode not in ('disabled', 'no-verification')
        assert so.pending == queued, 'request performed TLS I/O'
        try:
            s.verify_client_post_handshake()
        except ssl.SSLError:
            pass
        else:
            raise AssertionError('duplicate outstanding certificate request accepted')
        s.write(b'challenge')
        transfer(so, ci)
        assert c.read() == b'challenge'
        c.write(b'response')
        transfer(co, si)
        try:
            assert s.read() == b'response'
        except ssl.SSLError as error:
            assert mode in ('missing-required', 'untrusted')
            if mode == 'untrusted':
                assert isinstance(error, ssl.SSLCertVerificationError) and error.verify_code
            transfer(so, ci)
            try:
                c.read()
            except ssl.SSLError:
                return 'post-auth-rejected'
            raise AssertionError('authentication failure alert not delivered')
        assert mode in ('cert', 'optional')
        if mode == 'cert':
            assert s.getpeercert(binary_form=True) and s.getpeercert()['subject']
        else:
            assert s.getpeercert() is None
        return 'post-authenticated'
    finally:
        dispose(pair)


def custom_bio_classes(cert_dir):
    import weakref
    created = []
    class Custom(ssl.SSLObject):
        def __init__(self, *args, **kwargs):
            raise AssertionError('public constructor used')
        @classmethod
        def _create(cls, incoming, outgoing, **kwargs):
            created.append((kwargs['server_side'], kwargs['server_hostname']))
            return super()._create(incoming, outgoing, **kwargs)
        def write(self, data):
            self.writes = getattr(self, 'writes', 0) + 1
            return super().write(data)
    class CustomContext(ssl.SSLContext):
        sslobject_class = Custom
    client = CustomContext(ssl.PROTOCOL_TLS_CLIENT)
    client.load_verify_locations(cafile=cert_dir + '/server.crt')
    client.application_state = {'custom': True}
    assert weakref.ref(client)() is client and client.application_state['custom']
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    assert server.sslobject_class is ssl.SSLObject and server.sslsocket_class is ssl.SSLSocket
    server.sslobject_class = Custom
    server.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    callbacks = []
    def choose(peer, name, context):
        assert type(peer) is Custom and context is server
        callbacks.append(name)
    server.sni_callback = choose
    ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
    pair = client.wrap_bio(ci, co, server_hostname=b'localhost'), server.wrap_bio(si, so, server_side=True), ci, co, si, so
    try:
        c, s = pair[:2]
        assert type(c) is type(s) is Custom
        assert created == [(False, 'localhost'), (True, None)]
        handshake(pair)
        assert callbacks == ['localhost']
        c.write(b'custom BIO')
        transfer(co, si)
        assert s.read() == b'custom BIO' and c.writes == 1
    finally:
        dispose(pair)
    try:
        ssl.SSLObject()
    except TypeError:
        pass
    else:
        raise AssertionError('public SSLObject constructor accepted')
    class Failed(ssl.SSLObject):
        def __setattr__(self, name, value):
            if name in ('_sslobj', '_connection'):
                raise RuntimeError('custom initialization failed')
            return super().__setattr__(name, value)
    client.sslobject_class = Failed
    ci, co = ssl.MemoryBIO(), ssl.MemoryBIO()
    try:
        client.wrap_bio(ci, co, server_hostname='localhost')
    except RuntimeError:
        pass
    else:
        raise AssertionError('failing subclass initialized')
    client.sslobject_class = ssl.SSLObject
    retry = client.wrap_bio(ci, co, server_hostname='localhost')
    close = getattr(retry, 'close', None)
    if close is not None:
        close()
    return 'custom-bio-classes'


def live_tls13_settings(cert_dir, mode):
    client = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    if mode != 'tickets':
        client.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
        client.post_handshake_auth = mode == 'server-pha'
        server.post_handshake_auth = mode == 'client-pha'
        server.verify_mode = ssl.CERT_OPTIONAL
    def make_pair(session=None):
        ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
        return (client.wrap_bio(ci, co, server_hostname='localhost', session=session),
                server.wrap_bio(si, so, server_side=True), ci, co, si, so)
    def ticket_data(pair):
        pair[1].write(b'ticket data')
        transfer(pair[5], pair[2])
        assert pair[0].read() == b'ticket data'
    previous = make_pair()
    current = resumed = None
    try:
        if mode == 'tickets':
            server.num_tickets = 0
        else:
            target = client if mode == 'client-pha' else server
            target.post_handshake_auth = True
            server.load_verify_locations(cafile=cert_dir + '/server.crt')
        client.load_verify_locations(cafile=cert_dir + '/server.crt')
        handshake(previous)
        assert client.session_stats()['connect_good'] == server.session_stats()['accept_good'] == 1
        if mode == 'tickets':
            ticket_data(previous)
            saved = previous[0].session
            assert saved.has_ticket
            current = make_pair()
            handshake(current)
            ticket_data(current)
            assert not current[0].session.has_ticket
            server.num_tickets = 3
            resumed = make_pair(saved)
            handshake(resumed)
            ticket_data(resumed)
            assert resumed[0].session_reused and resumed[1].session_reused
            assert client.session_stats()['hits'] == server.session_stats()['hits'] == 1
            assert server.session_stats()['accept_good'] == 3
        else:
            assert bool(previous[1].getpeercert()) is (mode == 'server-pha')
            try:
                previous[1].verify_client_post_handshake()
            except ssl.SSLError:
                assert mode == 'client-pha'
            else:
                # Server PHA controls deferred INITIAL authentication; a
                # PEER server may later request if the client advertised PHA.
                assert mode == 'server-pha'
            current = make_pair()
            handshake(current)
            assert current[1].getpeercert() is None
            current[1].verify_client_post_handshake()
            current[1].write(b'challenge')
            transfer(current[5], current[2])
            assert current[0].read() == b'challenge'
            current[0].write(b'response')
            transfer(current[3], current[4])
            assert current[1].read() == b'response'
            assert current[1].getpeercert()['subject']
            assert client.session_stats()['connect_good'] == server.session_stats()['accept_good'] == 2
            target.post_handshake_auth = False
            assert client.session_stats()['connect_good'] == server.session_stats()['accept_good'] == 2
    finally:
        for pair in (previous, current, resumed):
            if pair is not None:
                dispose(pair)
    return 'live-tls13-settings'


def tls13_settings(cert_dir):
    class Index:
        def __index__(self):
            return 3
    for protocol in (ssl.PROTOCOL_TLS_CLIENT, ssl.PROTOCOL_TLS_SERVER):
        context = ssl.SSLContext(protocol)
        assert context.post_handshake_auth is False and context.num_tickets == 2
        for value in (None, 0, 1, 'enabled', []):
            context.post_handshake_auth = value
            assert context.post_handshake_auth is bool(value)
        for value, error in ((-1, ValueError), (1 << 63, OverflowError), (-(1 << 63) - 1, OverflowError), (1.5, TypeError)):
            try:
                context.num_tickets = value
            except error:
                pass
            else:
                raise AssertionError('invalid ticket count accepted')
        if protocol == ssl.PROTOCOL_TLS_CLIENT:
            try:
                context.num_tickets = 0
            except ValueError:
                pass
            else:
                raise AssertionError('client ticket count accepted')
        else:
            context.num_tickets = Index()
            assert context.num_tickets == 3
    for count in (0, 3):
        pair = tls_pair(cert_dir)
        c, s = pair[:2]
        client, server = c.context, s.context
        dispose(pair)
        server.num_tickets = count
        ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
        pair = client.wrap_bio(ci, co, server_hostname='localhost'), server.wrap_bio(si, so, server_side=True), ci, co, si, so
        c, s = pair[:2]
        try:
            handshake(pair)
            s.write(b'tickets')
            transfer(so, ci)
            assert c.read() == b'tickets'
            assert c.session.has_ticket is bool(count)
        finally:
            dispose(pair)
    return 'tls13-settings'


def sni_settings():
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    client = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    assert server.sni_callback is None
    callback = lambda *args: None
    server.sni_callback = callback
    assert server.sni_callback is callback
    try:
        server.sni_callback = 1
    except TypeError:
        pass
    else:
        raise AssertionError('noncallable SNI callback accepted')
    assert server.sni_callback is None
    server.sni_callback = callback
    try:
        server.set_servername_callback(1)
    except TypeError:
        pass
    else:
        raise AssertionError('noncallable legacy SNI callback accepted')
    assert server.sni_callback is callback
    server.set_servername_callback(None)
    assert server.sni_callback is None
    for value in (None, callback, 1):
        try:
            client.sni_callback = value
        except ValueError:
            pass
        else:
            raise AssertionError('client context accepted SNI callback')
    assert isinstance(ssl.ALERT_DESCRIPTION_UNRECOGNIZED_NAME, ssl.AlertDescription)
    assert int(ssl.ALERT_DESCRIPTION_UNRECOGNIZED_NAME) == 112
    assert int(ssl.ALERT_DESCRIPTION_INTERNAL_ERROR) == 80
    return 'sni-settings'


def ca_directory(cert_dir, temp_dir, mode='good'):
    import os
    from pathlib import Path
    directory = temp_dir + '/capath-' + mode
    os.makedirs(directory)
    def put(name, fixture, suffix=b''):
        with open(cert_dir + '/' + fixture, 'rb') as source:
            data = source.read()
        with open(directory + '/' + name, 'wb') as target:
            target.write(data + suffix)
    ca_modes = ('crl-good', 'crl-revoked', 'crl-gap', 'aux-rejected', 'mutual')
    if mode in ca_modes:
        put('701ade87.0', 'rejected-ca.pem' if mode == 'aux-rejected' else 'ca.crt')
        if mode.startswith('crl-'):
            put('701ade87.r1' if mode == 'crl-gap' else '701ade87.r0',
                'revoked.crl' if mode == 'crl-revoked' else 'empty.crl')
        if mode == 'mutual':
            put('ce275665.0', 'server.crt')
    elif mode == 'gap':
        put('ce275665.1', 'server.crt')
    elif mode == 'wrong-hash':
        put('deadbeef.0', 'server.crt')
    elif mode == 'plain-name':
        put('server.crt', 'server.crt')
    elif mode == 'malformed':
        with open(directory + '/ce275665.0', 'wb') as file:
            file.write(b'not a certificate')
        put('ce275665.1', 'server.crt')
    elif mode == 'collision':
        put('ce275665.0', 'ca.crt')
        put('ce275665.1', 'server.crt')
    elif mode != 'missing':
        put('ce275665.0', 'server.crt',
            b'-----BEGIN X509 CRL-----\ninvalid\n-----END X509 CRL-----\n' if mode == 'mixed' else b'')
    client = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    capath = Path(directory) if mode == 'pathlike' else os.fsencode(directory) if mode == 'bytes' else directory
    if mode == 'multiple':
        capath = temp_dir + '/absent::' + directory
    elif mode == 'missing':
        capath = temp_dir + '/absent'
    client.load_verify_locations(capath=capath)
    client.load_verify_locations(capath=capath)  # Duplicate registration is benign.
    assert client.cert_store_stats() == {'x509': 0, 'x509_ca': 0, 'crl': 0}
    assert client.get_ca_certs() == []
    if mode.startswith('crl-'):
        client.verify_flags |= ssl.VERIFY_CRL_CHECK_LEAF
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server.load_cert_chain(cert_dir + ('/leaf.crt' if mode in ca_modes and mode != 'mutual' else '/server.crt'),
                           cert_dir + '/server.key')
    if mode == 'mutual':
        client.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
        server.load_verify_locations(capath=directory)
        server.verify_mode = ssl.CERT_REQUIRED
    ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
    pair = (client.wrap_bio(ci, co, server_hostname='localhost'),
            server.wrap_bio(si, so, server_side=True), ci, co, si, so)
    failed = mode in ('gap', 'wrong-hash', 'plain-name', 'malformed', 'missing',
                      'crl-revoked', 'crl-gap', 'aux-rejected')
    try:
        assert client.cert_store_stats()['x509'] == 0
        try:
            handshake(pair)
        except ssl.SSLCertVerificationError as error:
            assert failed and error.verify_code
            if mode == 'crl-revoked':
                assert error.verify_code == 23
            elif mode == 'crl-gap':
                assert error.verify_code == 3
        else:
            assert not failed
            pair[0].write(b'directory trust')
            transfer(co, si)
            assert pair[1].read(100) == b'directory trust'
        stats = client.cert_store_stats()
        expected = 0 if mode in ('gap', 'wrong-hash', 'plain-name', 'malformed', 'missing') else 2 if mode == 'collision' else 1
        assert stats['x509'] == stats['x509_ca'] == expected
        assert stats['crl'] == (1 if mode in ('crl-good', 'crl-revoked') else 0)
        assert len(client.get_ca_certs()) == len(client.get_ca_certs(True)) == expected
        if mode == 'mutual':
            assert server.cert_store_stats()['x509'] == 1
        # Loaded issuers remain cached after the TLS connection is released.
        dispose(pair)
        assert client.cert_store_stats() == stats
    finally:
        dispose(pair)
    return 'ca-directory'


def security_level_inheritance(cert_dir, cached):
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    if cached:
        ctx.session_stats()
    ctx.set_ciphers('DEFAULT:@SECLEVEL=1')
    assert ctx.security_level == 1
    ctx.set_ciphers('ECDHE-RSA-AES128-GCM-SHA256')
    assert ctx.security_level == 1
    assert [value['name'] for value in ctx.get_ciphers() if value['protocol'] != 'TLSv1.3'] == ['ECDHE-RSA-AES128-GCM-SHA256']
    ctx.options ^= ssl.OP_CIPHER_SERVER_PREFERENCE
    assert ctx.security_level == 1
    ctx.load_dh_params(cert_dir + '/dh1024.pem')
    # Applying a setting still using snapshots must retain the inherited level.
    ctx.session_stats()
    assert ctx.security_level == 1
    ctx.set_ciphers('ECDHE-RSA-AES128-GCM-SHA256:@SECLEVEL=2')
    assert ctx.security_level == 2
    try:
        ctx.load_dh_params(cert_dir + '/dh1024.pem')
    except ssl.SSLError:
        pass
    else:
        raise AssertionError('DH1024 accepted at security level 2')
    return 'security-level-inheritance'


def suiteb_policy_inheritance(cert_dir, cached):
    names = ('ECDHE-ECDSA-AES128-GCM-SHA256', 'ECDHE-ECDSA-AES256-GCM-SHA384')
    for mode, expected, retained in (
        ('SUITEB128', names, names),
        ('SUITEB128ONLY', names[:1], names[:1]),
        ('SUITEB128C2', names[1:], names),
        ('SUITEB192', names[1:], names[1:]),
    ):
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        if cached:
            ctx.session_stats()
        ctx.set_ciphers('DEFAULT:@SECLEVEL=1')
        # Suite B ignores suffix commands, including a later SECLEVEL.
        ctx.set_ciphers(mode + ':@SECLEVEL=2:not-a-cipher')
        assert ctx.security_level == 1
        current = lambda: tuple(c['name'] for c in ctx.get_ciphers() if c['protocol'] != 'TLSv1.3')
        assert current() == expected, (mode, current())
        ctx.options ^= ssl.OP_CIPHER_SERVER_PREFERENCE
        assert current() == expected
        # The native certificate flags persist across ordinary list changes.
        ctx.set_ciphers('not-a-cipher:@SECLEVEL=2')
        assert current() == retained, (mode, current())
        assert ctx.security_level == 1
        ctx.load_dh_params(cert_dir + '/dh1024.pem')
        ctx.session_stats()
        assert current() == retained
        assert ctx.security_level == 1
    return 'suiteb-policy-inheritance'


def live_connection_settings(cert_dir, field):
    client = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    if field in ('verify-on', 'check-on'):
        client.check_hostname = False
        client.verify_mode = ssl.CERT_NONE
    elif field == 'minimum':
        server.maximum_version = ssl.TLSVersion.TLSv1_2
    name = 'mismatch.example' if field == 'check-on' else 'localhost'
    def make_pair():
        ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
        return (client.wrap_bio(ci, co, server_hostname=name),
                server.wrap_bio(si, so, server_side=True), ci, co, si, so)
    previous = make_pair()
    current = None
    try:
        if field == 'maximum':
            client.maximum_version = ssl.TLSVersion.TLSv1_2
        elif field == 'minimum':
            client.minimum_version = ssl.TLSVersion.TLSv1_3
        elif field == 'inverted':
            client.minimum_version = ssl.TLSVersion.TLSv1_3
            client.maximum_version = ssl.TLSVersion.TLSv1_2
        elif field == 'verify-on':
            client.verify_mode = ssl.CERT_REQUIRED
        elif field == 'verify-off':
            client.check_hostname = False
            client.verify_mode = ssl.CERT_NONE
        elif field == 'check-on':
            client.check_hostname = True
            assert client.verify_mode == ssl.CERT_REQUIRED
        else:
            raise AssertionError(field)
        assert client.session_stats()['connect_good'] == 0
        # Changes must retain the native context/store of the older SSL.
        if field != 'verify-on':
            client.load_verify_locations(cafile=cert_dir + '/server.crt')
        handshake(previous)
        assert previous[0].version() == ('TLSv1.2' if field == 'minimum' else 'TLSv1.3')
        # CURRENT parent-context mode controls decoded/empty output, while
        # verification itself uses flags copied into the existing SSL.
        assert bool(previous[0].getpeercert()) is (field != 'verify-off')
        assert client.session_stats()['connect_good'] == 1
        current = make_pair()  # inverted bounds fail at handshake, not wrapping
        try:
            handshake(current)
        except ssl.SSLCertVerificationError as error:
            assert field in ('verify-on', 'check-on')
            assert error.verify_code == (18 if field == 'verify-on' else 62)
        except ssl.SSLError:
            assert field in ('minimum', 'inverted')
        else:
            assert field in ('maximum', 'verify-off')
            assert current[0].version() == ('TLSv1.2' if field == 'maximum' else 'TLSv1.3')
        assert client.session_stats()['connect_good'] == (2 if field in ('maximum', 'verify-off') else 1)
        previous[0].write(b'retained connection settings')
        transfer(previous[3], previous[4])
        assert previous[1].read(100) == b'retained connection settings'
    finally:
        dispose(previous)
        if current is not None:
            dispose(current)
    return 'live-connection-settings'


def live_context_policy(cert_dir, field):
    client = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    cert = '/common-name.crt' if field == 'common-name' else '/leaf.crt' if field == 'verify-flags' else '/server.crt'
    server.load_cert_chain(cert_dir + cert, cert_dir + '/server.key')
    if field == 'ciphers':
        client.maximum_version = server.maximum_version = ssl.TLSVersion.TLSv1_2
        client.set_ciphers('ECDHE-RSA-AES128-GCM-SHA256')
    def make_pair():
        ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
        return (client.wrap_bio(ci, co, server_hostname='localhost'),
                server.wrap_bio(si, so, server_side=True), ci, co, si, so)
    previous = make_pair()
    current = None
    try:
        if field == 'options':
            import warnings
            with warnings.catch_warnings():
                warnings.simplefilter('ignore', DeprecationWarning)
                client.options |= ssl.OP_NO_TLSv1_3
        elif field == 'verify-flags':
            client.verify_flags |= ssl.VERIFY_CRL_CHECK_LEAF
        elif field == 'common-name':
            client.hostname_checks_common_name = False
        else:
            client.set_ciphers('ECDHE-RSA-AES256-GCM-SHA384')
        assert client.session_stats()['connect_good'] == 0
        # Trust added after a policy update must reach the original SSL/store.
        client.load_verify_locations(cafile=cert_dir + ('/ca.crt' if field == 'verify-flags' else cert))
        handshake(previous)
        assert previous[0].version() == ('TLSv1.2' if field == 'ciphers' else 'TLSv1.3')
        if field == 'ciphers':
            assert previous[0].cipher()[0] == 'ECDHE-RSA-AES256-GCM-SHA384'
        assert client.session_stats()['connect_good'] == 1
        current = make_pair()
        try:
            handshake(current)
        except ssl.SSLCertVerificationError as error:
            assert field in ('verify-flags', 'common-name')
            assert error.verify_code == (3 if field == 'verify-flags' else 62)
        else:
            assert field in ('options', 'ciphers')
            assert current[0].version() == 'TLSv1.2'
            if field == 'ciphers':
                assert current[0].cipher()[0] == 'ECDHE-RSA-AES256-GCM-SHA384'
        assert client.cert_store_stats()['x509'] == 1
        assert client.session_stats()['connect_good'] == (1 if field in ('verify-flags', 'common-name') else 2)
        previous[0].write(b'policy and retained trust')
        transfer(previous[3], previous[4])
        assert previous[1].read(100) == b'policy and retained trust'
    finally:
        dispose(previous)
        if current is not None:
            dispose(current)
    return 'live-context-policy'


def live_trust_store(cert_dir, temp_dir, mode):
    import os
    directory = temp_dir + '/live-' + mode
    os.mkdir(directory)
    client = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    crl = mode == 'crl'
    mutual = mode == 'mutual'
    server.load_cert_chain(cert_dir + ('/leaf.crt' if crl else '/server.crt'), cert_dir + '/server.key')
    if mode == 'directory-change':
        client.load_verify_locations(capath=directory)
    if crl:
        client.load_verify_locations(cafile=cert_dir + '/ca.crt')
        client.verify_flags |= ssl.VERIFY_CRL_CHECK_LEAF
    if mutual:
        client.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
        server.verify_mode = ssl.CERT_REQUIRED
    ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
    pair = (client.wrap_bio(ci, co, server_hostname='localhost'),
            server.wrap_bio(si, so, server_side=True), ci, co, si, so)
    try:
        try:
            pair[0].do_handshake()
        except ssl.SSLWantReadError:
            pass
        else:
            raise AssertionError('client completed without a server')
        if mode in ('directory-change', 'directory-new'):
            with open(cert_dir + '/server.crt', 'rb') as source:
                data = source.read()
            with open(directory + '/ce275665.0', 'wb') as target:
                target.write(data)
            if mode == 'directory-new':
                client.load_verify_locations(capath=directory)
            assert client.cert_store_stats()['x509'] == 0
        elif mode == 'cadata':
            with open(cert_dir + '/server.crt') as source:
                client.load_verify_locations(cadata=source.read())
        elif crl:
            client.load_verify_locations(cafile=cert_dir + '/empty.crl')
        else:
            client.load_verify_locations(cafile=cert_dir + '/server.crt')
        if mutual:
            server.load_verify_locations(cafile=cert_dir + '/server.crt')
        handshake(pair)
        pair[0].write(b'live trust')
        transfer(co, si)
        assert pair[1].read(100) == b'live trust'
        stats = client.cert_store_stats()
        assert stats['x509'] == 1 and stats['crl'] == int(crl)
        # A malformed subsequent addition leaves the active trust/cache intact.
        try:
            client.load_verify_locations(cadata='not a PEM certificate')
        except ssl.SSLError:
            pass
        else:
            raise AssertionError('invalid trust accepted')
        assert client.cert_store_stats() == stats
        if mode.startswith('directory-'):
            os.unlink(directory + '/ce275665.0')
            # Cached native anchors remain after a guest directory changes.
            pair[0].write(b'cached anchor')
            transfer(co, si)
            assert pair[1].read(100) == b'cached anchor'
            assert client.cert_store_stats() == stats
    finally:
        dispose(pair)
    return 'live-trust-store'


def ca_directory_settings():
    for value, kind in (('', ssl.SSLError), (1, TypeError), ('bad\x00path', ValueError),
                        (b'bad\x00path', ValueError)):
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        try:
            ctx.load_verify_locations(capath=value)
        except kind:
            pass
        else:
            raise AssertionError('invalid CA directory accepted')
        assert ctx.cert_store_stats()['x509'] == 0
    ctx.load_verify_locations(capath=':')
    ctx.load_verify_locations(capath='nonexistent-path')
    assert ctx.get_ca_certs() == []
    return 'ca-directory-settings'


def compression_and_npn(cert_dir):
    import warnings
    assert ssl.HAS_NPN is False
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    for protocols, kind in (([], AttributeError), (['h2'], AttributeError),
                            ([''], ssl.SSLError), (['x' * 256], ssl.SSLError),
                            (['é'], UnicodeEncodeError), ([b'h2'], TypeError),
                            ([1], TypeError), (None, TypeError)):
        with warnings.catch_warnings(record=True) as seen:
            warnings.simplefilter('always')
            try:
                ctx.set_npn_protocols(protocols)
            except kind:
                pass
            else:
                raise AssertionError('unexpected NPN configuration success')
            assert len(seen) == 1 and seen[0].category is DeprecationWarning
    class HookedContext(ssl.SSLContext):
        def _set_npn_protocols(self, encoded):
            self.encoded = encoded
    ctx = HookedContext(ssl.PROTOCOL_TLS_CLIENT)
    with warnings.catch_warnings():
        warnings.simplefilter('ignore', DeprecationWarning)
        ctx.set_npn_protocols(['h2', 'http/1.1'])
    assert ctx.encoded == b'\x02h2\x08http/1.1'
    for maximum in (None, ssl.TLSVersion.TLSv1_2):
        pair = tls_pair(cert_dir, maximum=maximum)
        try:
            for handshaken in (False, True):
                if handshaken:
                    handshake(pair)
                for peer in pair[:2]:
                    assert peer.compression() is None
                    with warnings.catch_warnings(record=True) as seen:
                        warnings.simplefilter('always')
                        assert peer.selected_npn_protocol() is None
                        assert len(seen) == 1 and seen[0].category is DeprecationWarning
        finally:
            dispose(pair)
    return 'compression-and-npn'


def purpose_factories(cert_dir):
    for purpose, protocol in ((ssl.Purpose.SERVER_AUTH.value, ssl.PROTOCOL_TLS_CLIENT),
                              (ssl.Purpose.CLIENT_AUTH.value, ssl.PROTOCOL_TLS_SERVER)):
        ctx = ssl.create_default_context(purpose, cafile=cert_dir + '/ca.crt')
        assert ctx.protocol == protocol
        ctx = ssl._create_unverified_context(purpose=purpose)
        assert ctx.protocol == protocol and ctx.verify_mode == ssl.CERT_NONE
        ctx.load_default_certs(purpose)
    other = ssl._ASN1Object.fromname('rsaEncryption')
    ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT).load_default_certs(other)
    for factory in (ssl.create_default_context, ssl._create_unverified_context):
        for purpose, kind in ((None, TypeError), (1, TypeError), (other, ValueError)):
            try:
                factory(purpose=purpose)
            except kind:
                pass
            else:
                raise AssertionError('invalid purpose accepted')
    for kwargs in ({'keyfile': 'key.pem'}, {'check_hostname': True, 'cert_reqs': ssl.CERT_NONE}):
        try:
            ssl._create_unverified_context(**kwargs)
        except ValueError:
            pass
        else:
            raise AssertionError('invalid unverified factory settings accepted')
    ctx = ssl._create_unverified_context(cert_reqs=None)
    assert ctx.verify_mode == ssl.CERT_REQUIRED and ctx.check_hostname is False
    for certfile in ('', 0, False):
        ctx = ssl._create_unverified_context(certfile=certfile)
        assert ctx.verify_mode == ssl.CERT_NONE
    return 'purpose-factories'


def message_pair(cert_dir, tls12=False, client_callback=None, server_callback=None):
    old = tls_pair(cert_dir, maximum=ssl.TLSVersion.TLSv1_2 if tls12 else None)
    client, server = old[0].context, old[1].context
    dispose(old)
    client._msg_callback, server._msg_callback = client_callback, server_callback
    ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
    return (client.wrap_bio(ci, co, server_hostname='localhost'),
            server.wrap_bio(si, so, server_side=True), ci, co, si, so)


def message_exchange(cert_dir, tls12=False):
    events = []
    def callback(*args):
        events.append(args)
    pair = message_pair(cert_dir, tls12, callback, callback)
    client, server = pair[:2]
    try:
        handshake(pair)
        for owner in (client, server):
            observed = [event for event in events if event[0] is owner]
            assert observed and {event[1] for event in observed} == {'read', 'write'}
            assert any(event[3] == ssl._TLSContentType.HANDSHAKE and
                       event[4] == ssl._TLSMessageType.CLIENT_HELLO for event in observed)
            assert any(event[3] == ssl._TLSContentType.HANDSHAKE and
                       event[4] == ssl._TLSMessageType.FINISHED for event in observed)
        for owner, direction, version, content, kind, data in events:
            assert type(version) is ssl.TLSVersion
            assert type(content) is ssl._TLSContentType and type(data) is bytes
            assert content != ssl._TLSContentType.APPLICATION_DATA
            if content == ssl._TLSContentType.HEADER:
                assert type(kind) is ssl._TLSContentType
                assert int(version) == int.from_bytes(data[1:3], 'big')
            elif content == ssl._TLSContentType.HANDSHAKE:
                assert type(kind) is ssl._TLSMessageType and int(kind) == data[0]
        events.clear()
        client.write(b'application secret')
        transfer(pair[3], pair[4])
        assert server.read(100) == b'application secret'
        assert all(b'application secret' not in event[5] for event in events)
        events.clear()
        for _ in range(8):
            done = 0
            for owner, source, target in ((client, pair[3], pair[4]), (server, pair[5], pair[2])):
                try:
                    owner.unwrap()
                    done += 1
                except ssl.SSLWantReadError:
                    pass
                transfer(source, target)
            if done == 2:
                break
        else:
            raise AssertionError('close_notify exchange did not finish')
        alerts = [event for event in events if event[3] == ssl._TLSContentType.ALERT]
        assert len(alerts) >= 2
        assert all(type(event[4]) is ssl._TLSAlertType and event[4] == ssl._TLSAlertType.CLOSE_NOTIFY
                   for event in alerts)
    finally:
        dispose(pair)
    return 'message-exchange'


def message_settings(cert_dir):
    events = []
    first = lambda *args: events.append('first')
    second = lambda *args: events.append('second')
    pair = message_pair(cert_dir, client_callback=first)
    try:
        ctx = pair[0].context
        assert ctx._msg_callback is first
        for value in (1, 'bad', object()):
            try:
                ctx._msg_callback = value
            except TypeError:
                pass
            else:
                raise AssertionError('non-callable message callback accepted')
            assert ctx._msg_callback is first
        ctx._msg_callback = second
        handshake(pair)
        assert events and set(events) == {'second'}
        ctx._msg_callback = None
        events.clear()
        pair[0].write(b'no diagnostics')
        assert not events
        ctx._msg_callback = first
        pair[0].write(b're-enabled')
        assert events and set(events) == {'first'}
    finally:
        dispose(pair)
    # A previously uninstalled SSL callback is not enabled by changing the
    # context alone. Explicit SSL.context assignment copies its installation
    # before the record layer is initialized by the handshake.
    events.clear()
    pair = message_pair(cert_dir)
    try:
        pair[0].context._msg_callback = first
        handshake(pair)
        assert not events
    finally:
        dispose(pair)
    pair = message_pair(cert_dir)
    try:
        pair[0].context._msg_callback = first
        pair[0].context = pair[0].context
        handshake(pair)
        assert events
    finally:
        dispose(pair)
    return 'message-settings'


def message_failures(cert_dir, locked=False):
    errors = []
    class LockedError(RuntimeError):
        def __setattr__(self, name, value):
            raise AttributeError('exception is immutable')
    def fail(*args):
        error = (LockedError if locked else RuntimeError)('message callback ' + str(len(errors)))
        errors.append(error)
        raise error
    pair = message_pair(cert_dir, client_callback=fail)
    client, server = pair[:2]
    try:
        try:
            client.do_handshake()
        except ssl.SSLWantReadError as error:
            assert errors and error.__context__ is errors[-1]
        else:
            raise AssertionError('callback replaced readiness')
        client.context._msg_callback = lambda *args: None
        transfer(pair[3], pair[4])
        handshake(pair)
        client.context._msg_callback = fail
        errors.clear()
        try:
            client.write(b'one completed write')
        except RuntimeError as error:
            assert errors and error is errors[-1]
        else:
            raise AssertionError('successful write hid callback failure')
        transfer(pair[3], pair[4])
        assert server.read(100) == b'one completed write'
        client.context._msg_callback = None
        assert client.pending() == 0
    finally:
        dispose(pair)
    return 'message-failures'


def message_queue_failures(cert_dir):
    pair = tls_pair(cert_dir)
    client, server = pair[:2]
    if not hasattr(client, '_connection'):
        dispose(pair)
        return 'message-queue-failures'
    connection = client._connection
    class OverflowedConnection:
        def __getattr__(self, name):
            return getattr(connection, name)
        def take_messages(self):
            connection.take_messages()
            raise ssl.SSLError(ssl.SSL_ERROR_SSL, 'injected message queue overflow')
    try:
        handshake(pair)
        client._connection = OverflowedConnection()
        try:
            client.read(100)
        except ssl.SSLWantReadError as error:
            assert isinstance(error.__context__, ssl.SSLError)
            assert 'queue overflow' in str(error.__context__)
        else:
            raise AssertionError('queue overflow replaced TLS readiness')
        try:
            client.write(b'one completed write')
        except ssl.SSLError as error:
            assert 'queue overflow' in str(error)
        else:
            raise AssertionError('queue overflow hidden after completed write')
        transfer(pair[3], pair[4])
        assert server.read(100) == b'one completed write'
    finally:
        client._connection = connection
        dispose(pair)
    return 'message-queue-failures'


def message_sni_switch(cert_dir):
    events = []
    replacement = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    replacement.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    replacement._msg_callback = lambda *args: events.append(('replacement', args[3], args[4]))
    pair = message_pair(cert_dir, server_callback=lambda *args: events.append(('original', args[3], args[4])))
    original = pair[1].context
    def choose(peer, name, context):
        events.append(('sni', name))
        peer.context = replacement
    original.sni_callback = choose
    try:
        handshake(pair)
        hello = next(i for i, event in enumerate(events) if len(event) == 3 and
                     event[1:] == (ssl._TLSContentType.HANDSHAKE, ssl._TLSMessageType.CLIENT_HELLO))
        sni = next(i for i, event in enumerate(events) if event[0] == 'sni')
        assert hello < sni and events[hello][0] == 'original'
        assert any(event[0] == 'replacement' for event in events[sni + 1:])
        assert not any(event[0] == 'original' for event in events[sni + 1:])
    finally:
        dispose(pair)
    return 'message-sni-switch'


def _keylog_lines(path):
    with open(path, 'rb') as file:
        content = file.read()
    lines = [line.split() for line in content.splitlines() if line and not line.startswith(b'#')]
    assert all(len(parts) == 3 and len(parts[1]) == 64 and len(parts[2]) in (64, 96)
               for parts in lines)
    for _, random, secret in lines:
        bytes.fromhex(random.decode('ascii'))
        bytes.fromhex(secret.decode('ascii'))
    return {tuple(line) for line in lines}


def keylog_exchange(cert_dir, temp_dir, tls12=False, switch=False):
    import os
    pair = tls_pair(cert_dir, maximum=ssl.TLSVersion.TLSv1_2 if tls12 else None)
    client, server = pair[:2]
    client_path, server_path = temp_dir + '/client.log', temp_dir + '/server.log'
    for path in (client_path, server_path, temp_dir + '/original.log'):
        try:
            os.unlink(path)
        except FileNotFoundError:
            pass
    client.context.keylog_filename = client_path
    server.context.keylog_filename = server_path
    original = server.context
    if switch:
        replacement = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        replacement.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
        replacement.keylog_filename = server_path
        original.keylog_filename = temp_dir + '/original.log'
        def choose(peer, name, context):
            peer.context = replacement
        original.sni_callback = choose
        dispose(pair)
        client = client.context.wrap_bio(pair[2], pair[3], server_hostname='localhost')
        server = original.wrap_bio(pair[4], pair[5], server_side=True)
        pair = (client, server, *pair[2:])
    try:
        handshake(pair)
        client.write(b'keylog exchange')
        transfer(pair[3], pair[4])
        assert server.read(100) == b'keylog exchange'
        client_lines, server_lines = _keylog_lines(client_path), _keylog_lines(server_path)
        assert client_lines == server_lines and client_lines
        labels = {line[0] for line in client_lines}
        if tls12:
            assert labels == {b'CLIENT_RANDOM'}
        else:
            assert {b'CLIENT_HANDSHAKE_TRAFFIC_SECRET', b'SERVER_HANDSHAKE_TRAFFIC_SECRET',
                    b'CLIENT_TRAFFIC_SECRET_0', b'SERVER_TRAFFIC_SECRET_0'} <= labels
        if switch:
            assert not _keylog_lines(temp_dir + '/original.log')
    finally:
        client.context.keylog_filename = server.context.keylog_filename = None
        original.keylog_filename = None
        dispose(pair)
    return 'keylog-exchange'


def keylog_failures(cert_dir):
    pair = tls_pair(cert_dir)
    client, server = pair[:2]
    if not hasattr(client, '_connection'):
        dispose(pair)
        return 'keylog-failures'
    connection = client._connection
    try:
        handshake(pair)
        connection = client._connection
        class LoggedConnection:
            def __getattr__(self, name):
                return getattr(connection, name)
            def take_keylog(self):
                connection.take_keylog()
                if mode == 'queue':
                    raise ssl.SSLError(ssl.SSL_ERROR_SSL, 'injected keylog overflow')
                return b'injected log line\n'
        class BrokenLog:
            def write(self, lines):
                if mode == 'write':
                    raise OSError('injected log write failure')
            def flush(self):
                raise OSError('injected log flush failure')
        client._connection = LoggedConnection()
        for mode in ('queue', 'write', 'flush'):
            client.context._keylog_file = BrokenLog()
            # Readiness remains the primary exception and the failed logging
            # operation is retained as its cause, including host queue overflow.
            try:
                client.read(10)
            except ssl.SSLWantReadError as error:
                assert isinstance(error.__cause__, ssl.SSLError if mode == 'queue' else OSError)
            else:
                raise AssertionError('logging replaced TLS readiness')
            # After success, CPython surfaces the logging exception. The actual
            # TLS write/read has completed: verify it once, without retrying it.
            expected = ssl.SSLError if mode == 'queue' else OSError
            try:
                client.write(b'one write')
            except expected:
                pass
            else:
                raise AssertionError('successful TLS write hid logging failure')
            transfer(pair[3], pair[4])
            assert server.read(100) == b'one write'
            server.write(b'one read')
            transfer(pair[5], pair[2])
            try:
                client.read(100)
            except expected:
                pass
            else:
                raise AssertionError('successful TLS read hid logging failure')
            client.context._keylog_file = None
            client._connection = connection
            assert client.pending() == 0
            client._connection = LoggedConnection()
        client.context._keylog_file = None
        client._connection = connection
        dispose(pair)
        # A certificate verification failure must also retain its own errno,
        # subclass, reason and verification code when logging fails.
        pair = tls_pair(cert_dir, name='wrong.example')
        client, server = pair[:2]
        connection = client._connection
        mode = 'write'
        client._connection = LoggedConnection()
        client.context._keylog_file = BrokenLog()
        try:
            handshake(pair)
        except ssl.SSLCertVerificationError as error:
            assert isinstance(error.__cause__, OSError) and error.verify_code
        else:
            raise AssertionError('logging replaced certificate verification failure')
    finally:
        client.context._keylog_file = None
        client._connection = connection
        dispose(pair)
    return 'keylog-failures'


def keylog_settings(cert_dir, temp_dir):
    import os
    from pathlib import Path
    path = temp_dir + '/settings.log'
    try:
        os.unlink(path)
    except FileNotFoundError:
        pass
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    assert ctx.keylog_filename is None
    selected = Path(path)
    ctx.keylog_filename = selected
    assert ctx.keylog_filename is selected
    with open(path, 'rb') as file:
        assert file.read() == b'# TLS secrets log file, generated by OpenSSL / Python\n'
    ctx.keylog_filename = os.fsencode(path)
    assert ctx.keylog_filename == os.fsencode(path)
    ctx.keylog_filename = None
    with open(path, 'rb') as file:
        assert len(file.read().splitlines()) == 1
    for value, kind in ((7, TypeError), ('\x00', ValueError), (temp_dir, IsADirectoryError),
                        (temp_dir + '/missing/settings.log', FileNotFoundError)):
        ctx.keylog_filename = path
        try:
            ctx.keylog_filename = value
        except kind:
            pass
        else:
            raise AssertionError('invalid key log filename accepted')
        assert ctx.keylog_filename is None
    before = os.environ.get('SSLKEYLOGFILE')
    try:
        os.environ['SSLKEYLOGFILE'] = path
        for factory in (ssl.create_default_context, ssl._create_unverified_context):
            created = factory()
            assert created.keylog_filename == path
            created.keylog_filename = None
    finally:
        if before is None:
            os.environ.pop('SSLKEYLOGFILE', None)
        else:
            os.environ['SSLKEYLOGFILE'] = before
    pair = tls_pair(cert_dir)
    try:
        pair[0].context.keylog_filename = path
        pair[0].context.keylog_filename = None
        handshake(pair)
        assert not _keylog_lines(path)
    finally:
        dispose(pair)
    return 'keylog-settings'


def module_metadata():
    assert isinstance(ssl.PROTOCOL_TLS_CLIENT, ssl._SSLMethod)
    assert ssl.PROTOCOL_SSLv23 is ssl.PROTOCOL_TLS
    assert ssl.SSLContext(16).protocol is ssl.PROTOCOL_TLS_CLIENT
    assert ssl.get_protocol_name(16) == 'PROTOCOL_TLS_CLIENT'
    assert ssl.get_protocol_name(ssl.PROTOCOL_TLS) == 'PROTOCOL_TLS'
    assert ssl.get_protocol_name(123456) == '<unknown>'
    expected_errors = dict(SSL_ERROR_SSL=1, SSL_ERROR_WANT_READ=2, SSL_ERROR_WANT_WRITE=3,
                           SSL_ERROR_WANT_X509_LOOKUP=4, SSL_ERROR_SYSCALL=5,
                           SSL_ERROR_ZERO_RETURN=6, SSL_ERROR_WANT_CONNECT=7,
                           SSL_ERROR_EOF=8, SSL_ERROR_INVALID_ERROR_CODE=10)
    assert {name: int(value) for name, value in ssl.SSLErrorNumber.__members__.items()} == expected_errors
    assert issubclass(ssl.SSLSyscallError, ssl.SSLError) and ssl.HAS_ECDH
    for purpose, nid, shortname, longname, oid in (
        (ssl.Purpose.SERVER_AUTH, 129, 'serverAuth', 'TLS Web Server Authentication', '1.3.6.1.5.5.7.3.1'),
        (ssl.Purpose.CLIENT_AUTH, 130, 'clientAuth', 'TLS Web Client Authentication', '1.3.6.1.5.5.7.3.2')):
        expected = (nid, shortname, longname, oid)
        assert tuple(purpose) == expected and tuple(purpose.value) == expected
        assert (purpose.nid, purpose.shortname, purpose.longname, purpose.oid) == expected
        assert ssl.Purpose(purpose.value) is purpose
        assert ssl._ASN1Object(oid) == purpose.value
        assert ssl._ASN1Object.fromname(shortname) == purpose.value
        assert ssl._ASN1Object.fromname(longname) == purpose.value
        assert ssl._ASN1Object.fromnid(nid) == purpose.value
    for function, arg, kind in (
        (ssl._ASN1Object, 'serverAuth', ValueError),
        (ssl._ASN1Object, '1.3.6.1.4.1.55555.1', ValueError),
        (ssl._ASN1Object.fromname, 'unknown-object', ValueError),
        (ssl._ASN1Object.fromnid, 0, ValueError),
        (ssl._ASN1Object.fromnid, -1, ValueError),
        (ssl._ASN1Object.fromnid, 2147483647, ValueError),
        (ssl._ASN1Object.fromnid, 1 << 31, OverflowError),
        (ssl._ASN1Object.fromnid, 1.5, TypeError),
        (ssl._ASN1Object.fromname, b'serverAuth', TypeError),
        (ssl._ASN1Object.fromname, 'serverAuth\x00', ValueError)):
        try:
            function(arg)
        except kind:
            pass
        else:
            raise AssertionError('invalid ASN.1 lookup accepted')
    assert ssl._ASN1Object.fromname('rsaEncryption').oid == '1.2.840.113549.1.1.1'
    return 'module-metadata'


def random_helpers():
    assert ssl.RAND_status() is True
    class Index:
        def __index__(self):
            return 257
    for size in (0, 1, 1024 * 1024, Index()):
        data = ssl.RAND_bytes(size)
        assert isinstance(data, bytes) and len(data) == (257 if isinstance(size, Index) else size)
    for size, kind in ((-1, ValueError), (1.0, TypeError), ('1', TypeError), (1 << 31, OverflowError)):
        try:
            ssl.RAND_bytes(size)
        except kind:
            pass
        else:
            raise AssertionError('invalid random byte count accepted')
    for data in ('é', b'', b'input', bytearray(b'input'), memoryview(b'input')):
        assert ssl.RAND_add(data, 0.0) is None
    for entropy in (float('nan'), float('inf'), -1.0):
        assert ssl.RAND_add(b'input', entropy) is None
    class NumericText(str):
        def __float__(self):
            return 0.5
    class NumericBytes(bytes):
        def __index__(self):
            return 1
    class FloatSubclass(float):
        def __float__(self):
            raise AssertionError('float subclass hook must be bypassed')
    for entropy in (NumericText('not a number'), NumericBytes(b'not a number'), FloatSubclass(0.0), True):
        assert ssl.RAND_add(b'input', entropy) is None
    for data, entropy, kind in ((None, 0, TypeError), (b'a', '1', TypeError),
                               (b'a', None, TypeError), (b'a', memoryview(b'1'), TypeError),
                               (b'a', memoryview(b'a'), TypeError),
                               (memoryview(b'abcd')[::2], 0, BufferError)):
        try:
            ssl.RAND_add(data, entropy)
        except kind:
            pass
        else:
            raise AssertionError('invalid random input accepted')
    return 'random-helpers'


def default_verify_paths(cert_dir):
    defaults = ssl.get_default_verify_paths()
    assert isinstance(defaults, ssl.DefaultVerifyPaths)
    assert defaults._fields == ('cafile', 'capath', 'openssl_cafile_env', 'openssl_cafile',
                                'openssl_capath_env', 'openssl_capath')
    assert defaults.openssl_cafile_env == 'SSL_CERT_FILE' and defaults.openssl_capath_env == 'SSL_CERT_DIR'
    assert isinstance(defaults.openssl_cafile, str) and isinstance(defaults.openssl_capath, str)
    import os
    original = {name: os.environ.get(name) for name in ('SSL_CERT_FILE', 'SSL_CERT_DIR')}
    try:
        os.environ['SSL_CERT_FILE'] = cert_dir + '/ca.crt'
        os.environ['SSL_CERT_DIR'] = cert_dir
        selected = ssl.get_default_verify_paths()
        assert selected.cafile == cert_dir + '/ca.crt' and selected.capath == cert_dir
        assert selected[2:] == defaults[2:]
        os.environ['SSL_CERT_FILE'] = cert_dir + '/missing.pem'
        os.environ['SSL_CERT_DIR'] = ''
        selected = ssl.get_default_verify_paths()
        assert selected.cafile is None and selected.capath is None
    finally:
        for name, value in original.items():
            if value is None:
                os.environ.pop(name, None)
            else:
                os.environ[name] = value
    return 'default-verify-paths'


if __name__ == "__main__":
    cert_dir = sys.argv[1]
    print(symbolic_errors(cert_dir))
    print(unexpected_eof_options(cert_dir))
    print(verification_messages(cert_dir))
    print(defaults_and_memory_bio())
    print(exchange(cert_dir))
    print(exchange(cert_dir, tls12=True))
    print(rejection(cert_dir, name="wrong.example"))
    print(rejection(cert_dir, trusted=False))
    print(shutdown_and_eof(cert_dir))
    print(large_buffered_input(cert_dir))
    print(memory_bio_buffer_rules())
    print(hostname_argument_rules())
    for tls12 in (False, True):
        print(tls_buffer_rules(cert_dir, tls12))
        print(empty_write_handshake(cert_dir, tls12))
        print(empty_write_handshake(cert_dir, tls12, False))
    for verify in (ssl.CERT_NONE, ssl.CERT_OPTIONAL, ssl.CERT_REQUIRED):
        print(certificate_metadata(cert_dir, verify))
    print(certificate_metadata(cert_dir, mutual=True))
    print(certificate_helpers(cert_dir))
    print(private_module_constants())
    print(private_certificate_helpers(cert_dir))
    print(private_bio_arguments())
    print(private_bio_dead_owner(cert_dir))
    print(private_hostname_flags(cert_dir))
    for tls12 in (False, True):
        print(private_binary_alpn(cert_dir, tls12))
    for tls12 in (False, True):
        for supplied in (False, True):
            print(private_bio_exchange(cert_dir, tls12, supplied))
    for mutual in (False, True):
        for verified in (False, True):
            print(private_certificate_chains(cert_dir, mutual, verified))
    print(credential_formats(cert_dir))
    print(credential_load_errors(cert_dir))
    for tls12 in (False, True):
        for mode in ('client', 'server'):
            print(mode, tls12, live_credentials(cert_dir, mode, tls12))
    for cached in (False, True):
        print(cached, credential_security_history(cert_dir, cached))
        print(cached, credential_algorithms(cert_dir, cached))
        print(cached, protocol_bounds(cert_dir, cached))
        for fixed in (False, True):
            for tls11 in (False, True):
                print(tls11, fixed, cached, legacy_protocol_exchange(cert_dir, tls11, fixed, cached))
    print(legacy_policy_rejection(cert_dir))
    print(module_constants_and_star_import())
    for cached in (False, True):
        print(cached, raw_policy_masks(cert_dir, cached))
        print(cached, policy_mask_normalization(cert_dir, cached))
    print(policy_settings(cert_dir))
    for mode in ("cipher", "legacy-cipher-metadata", "disable-tls13", "disable-tls12", "common-name-enabled",
                 "common-name-disabled", "crl-required", "generic-tls", "fixed-tls12"):
        print(mode, policy_exchange(cert_dir, mode))
    print(store_introspection(cert_dir))
    print(store_pem_whitespace(cert_dir))
    print(connection_metadata(cert_dir))
    print(connection_metadata(cert_dir, True))
    for tls12 in (False, True):
        for setter in (False, True):
            print(tls12, setter, session_resumption(cert_dir, tls12, setter))
    print(session_errors(cert_dir))
    for mode in ("valid", "missing", "revoked", "aux-rejected"):
        print(mode, store_exchange(cert_dir, mode))

    print(key_exchange_settings(cert_dir))
    for tls12 in (False, True):
        for mode in ('client', 'server', 'enable', 'disable', 'no-overlap'):
            print(mode, tls12, live_alpn(cert_dir, mode, tls12))
    for mode in ('dh', 'curve'):
        print(mode, live_key_exchange(cert_dir, mode))
    for cached in (False, True):
        print(cached, dh_security_history(cert_dir, cached))
    for mode in ("dh", "dh-missing", "ecdh12", "ecdh13", "mismatch12", "mismatch13"):
        print(mode, key_exchange(cert_dir, mode))

    for tls12 in (False, True):
        for mode in ("switch", "idna", "legacy", "absent", "ip", "alert", "exception", "bad-return", "switch-alert", "pre-both", "pre-enable", "pre-disable", "hrr"):
            print(mode, tls12, sni_exchange(cert_dir, mode, tls12))
    print(sni_settings())
    for fixed, pha in ((False, False), (True, False), (False, True)):
        print(fixed, pha, generic_context_roles(cert_dir, fixed, pha))
    for tls12 in (False, True):
        for mode in ('enable', 'disable', 'replace', 'invalid', 'fallback-enable', 'fallback-disable',
                     'current-enable', 'current-disable', 'self-disable', 'self-alert'):
            print(mode, tls12, live_sni(cert_dir, mode, tls12))
    print(sni_acknowledgement(cert_dir))
    print(tls13_settings(cert_dir))
    for mode in ('tickets', 'client-pha', 'server-pha'):
        print(mode, live_tls13_settings(cert_dir, mode))
    print(custom_bio_classes(cert_dir))
    print(module_metadata())
    print(random_helpers())
    print(default_verify_paths(cert_dir))
    print(purpose_factories(cert_dir))
    print(compression_and_npn(cert_dir))
    print(keylog_failures(cert_dir))
    for tls12 in (False, True):
        print(message_exchange(cert_dir, tls12))
    print(message_settings(cert_dir))
    print(message_failures(cert_dir))
    print(message_failures(cert_dir, True))
    print(message_queue_failures(cert_dir))
    print(message_sni_switch(cert_dir))
    import tempfile
    print(ca_directory_settings())
    for field in ('options', 'verify-flags', 'common-name', 'ciphers'):
        print(field, live_context_policy(cert_dir, field))
    for field in ('maximum', 'minimum', 'inverted', 'verify-on', 'verify-off', 'check-on'):
        print(field, live_connection_settings(cert_dir, field))
    for cached in (False, True):
        print(cached, security_level_inheritance(cert_dir, cached))
        print(cached, suiteb_policy_inheritance(cert_dir, cached))
    with tempfile.TemporaryDirectory() as directory:
        print(default_trust_overrides(cert_dir, directory))
        for mode in ('cafile', 'cadata', 'directory-change', 'directory-new', 'crl', 'mutual'):
            print(mode, live_trust_store(cert_dir, directory, mode))
        for mode in ('good', 'pathlike', 'bytes', 'multiple', 'gap', 'wrong-hash', 'plain-name',
                     'malformed', 'collision', 'missing', 'mixed', 'crl-good', 'crl-revoked',
                     'crl-gap', 'aux-rejected', 'mutual'):
            print(mode, ca_directory(cert_dir, directory, mode))
    with tempfile.TemporaryDirectory() as temp_dir:
        print(keylog_settings(cert_dir, temp_dir))
        for tls12 in (False, True):
            print(keylog_exchange(cert_dir, temp_dir, tls12))
        print(keylog_exchange(cert_dir, temp_dir, switch=True))
    for mode in ('cert', 'optional', 'missing-required', 'untrusted', 'disabled', 'no-verification'):
        print(mode, post_handshake_auth(cert_dir, mode))
    print('TLS12', post_handshake_auth(cert_dir, 'optional', True))
