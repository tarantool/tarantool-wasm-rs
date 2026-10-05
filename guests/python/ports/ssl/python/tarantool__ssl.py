"""Python 3.12 certificate and module helpers over the guest TLS adapter.

This is a Python facade, not the CPython C extension ABI. Host imports are
lazy so the module can be frozen by componentize-py without invoking them.
"""
import os
import operator
import sys
import weakref
import tarantool_ssl as _ssl

ENCODING_PEM = 1
ENCODING_DER = 2
HOSTFLAG_ALWAYS_CHECK_SUBJECT = 1
HOSTFLAG_NO_WILDCARDS = 2
HOSTFLAG_NO_PARTIAL_WILDCARDS = 4
HOSTFLAG_MULTI_LABEL_WILDCARDS = 8
HOSTFLAG_SINGLE_LABEL_SUBDOMAINS = 16
HOSTFLAG_NEVER_CHECK_SUBJECT = 32
HAS_TLS_UNIQUE = True
for _name, _value in _ssl.TLSVersion.__members__.items():
    globals()['PROTO_' + _name] = int(_value)
_CONSTANT_NAMES = frozenset(name for name in dir(_ssl)
                          if not name.startswith('_') and name.replace('v', 'V').isupper())
_LAZY_CONSTANT_NAMES = (set(_ssl._PROTOCOL_FLAGS) |
                        {'OPENSSL_VERSION', 'OPENSSL_VERSION_NUMBER', 'OPENSSL_VERSION_INFO'})
# CPython's enum._convert_(source=_ssl) reads the module dictionary directly.
# Publish pure values now; querying host facts during componentization is invalid.
for _name in _CONSTANT_NAMES - _LAZY_CONSTANT_NAMES:
    _value = getattr(_ssl, _name)
    globals()[_name] = int(_value) if isinstance(_value, int) and not isinstance(_value, bool) else _value
MemoryBIO = _ssl.MemoryBIO
SSLSession = _ssl.SSLSession
SSLError = _ssl.SSLError
SSLZeroReturnError = _ssl.SSLZeroReturnError
SSLWantReadError = _ssl.SSLWantReadError
SSLWantWriteError = _ssl.SSLWantWriteError
SSLSyscallError = _ssl.SSLSyscallError
SSLEOFError = _ssl.SSLEOFError
SSLCertVerificationError = _ssl.SSLCertVerificationError
RAND_bytes = _ssl.RAND_bytes
RAND_status = _ssl.RAND_status
RAND_add = _ssl.RAND_add


def __getattr__(name):
    if name == '_DEFAULT_CIPHERS':
        value = _ssl._host.default_ciphers()
        globals()[name] = value
        return value
    if name == '_OPENSSL_API_VERSION':
        number = _ssl._host.compiled_api_version()
        value = ((number >> 28) & 15, (number >> 20) & 255,
                 (number >> 12) & 255, (number >> 4) & 255, number & 15)
        globals()[name] = value
        return value
    if name in _CONSTANT_NAMES:
        value = getattr(_ssl, name)
        value = int(value) if isinstance(value, int) and not isinstance(value, bool) else value
        globals()[name] = value
        return value
    raise AttributeError(name)


def __dir__():
    return sorted(set(globals()) | _CONSTANT_NAMES | {'_DEFAULT_CIPHERS', '_OPENSSL_API_VERSION'})


# Wildcard imports resolve lazy host facts only when executed at runtime.
__all__ = sorted(_CONSTANT_NAMES |
                 {name for name in globals() if not name.startswith('_') and name.replace('v', 'V').isupper()} |
                 {'Certificate', 'MemoryBIO', 'SSLSession', 'SSLError', 'SSLZeroReturnError',
                  'SSLWantReadError', 'SSLWantWriteError', 'SSLSyscallError', 'SSLEOFError',
                  'SSLCertVerificationError', 'RAND_bytes', 'RAND_status', 'RAND_add',
                  'get_default_verify_paths', 'txt2obj', 'nid2obj'})


def get_default_verify_paths():
    defaults = _ssl._invoke(_ssl._host.default_verify_paths)
    return (defaults.cafile_env, os.fsdecode(bytes(defaults.cafile)),
            defaults.capath_env, os.fsdecode(bytes(defaults.capath)))


def txt2obj(txt, name=False):
    # Clinic validates/encodes txt before converting the boolean argument.
    if not isinstance(txt, str):
        raise TypeError('txt must be str')
    if str.__contains__(txt, '\x00'):
        raise ValueError('embedded null character')
    str.encode(txt, 'utf-8')
    return tuple(_ssl._asn1_text(txt, not bool(name)))


def nid2obj(nid, /):
    return tuple(_ssl._ASN1Object.fromnid(nid))


def _test_decode_cert(path, /):
    path = os.fspath(path)
    # Validate the path before translating filesystem failures. In particular,
    # this helper must never interpret an integer as an open file descriptor.
    if (isinstance(path, str) and '\x00' in path or
            isinstance(path, bytes) and b'\x00' in path):
        raise ValueError('embedded null byte')
    try:
        with open(path, 'rb') as file:
            data = file.read(_ssl._LIMIT + 1)
    except OSError:
        raise SSLError("Can't open file") from None
    if len(data) > _ssl._LIMIT:
        raise SSLError('certificate input limit exceeded')
    try:
        details = _ssl._invoke(_ssl._host.decode_certificate, data)
    except (SSLError, ValueError):
        raise SSLError('Error decoding PEM-encoded file') from None
    return _ssl._certificate_dict(details)


class Certificate(metaclass=_ssl._ImmutableType):
    _immutable_type = True
    __slots__ = ('_certificate',)
    __module__ = '_ssl'

    def __new__(cls, *args, **kwargs):
        raise TypeError("cannot create '_ssl.Certificate' instances")

    def __init_subclass__(cls, **kwargs):
        raise TypeError("type '_ssl.Certificate' is not an acceptable base type")

    @classmethod
    def _create(cls, handle):
        self = object.__new__(cls)
        self._certificate = handle
        return self

    def get_info(self):
        return _ssl._certificate_dict(_ssl._invoke(self._certificate.get_info))

    def public_bytes(self, format=ENCODING_PEM):
        format = _ssl._signed_int(format)
        if format not in (1, 2, 0x101):
            raise ValueError('Unsupported format')
        result = bytes(_ssl._invoke(self._certificate.public_bytes, format))
        return result if format == ENCODING_DER else result.decode('ascii')

    def __repr__(self):
        subject = _ssl._invoke(self._certificate.subject)
        return "<_ssl.Certificate '%s'>" % subject

    def __hash__(self):
        value = _ssl._invoke(self._certificate.subject_hash)
        # CPython casts the unsigned native subject hash to Py_hash_t. Wasm
        # uses 32-bit Py_hash_t; -1 is reserved by the Python hash protocol.
        if sys.maxsize == (1 << 31) - 1 and value >= 1 << 31:
            value -= 1 << 32
        return -2 if value == -1 else value

    def __eq__(self, other):
        if type(other) is not Certificate:
            return NotImplemented
        return _ssl._invoke(self._certificate.equal, other._certificate)

    def __ne__(self, other):
        equal = self.__eq__(other)
        return NotImplemented if equal is NotImplemented else not equal

    def _release(self):
        handle = getattr(self, '_certificate', None)
        if handle is not None:
            self._certificate = None
            handle.__exit__(None, None, None)

    def __del__(self):
        try:
            self._release()
        except Exception:
            pass


def _chain(connection, verified):
    # Allocate the result before acquiring resources from the host.
    certificates = list()
    handles = _ssl._invoke(connection.certificate_chain, verified)
    if handles is None:
        return None
    current = None
    transferred = 0
    try:
        for handle in handles:
            current = Certificate._create(handle)
            transferred += 1
            certificates.append(current)
            current = None
    except BaseException:
        # A retained exception traceback may keep these wrappers alive. Close
        # them now, with ownership cleared first so collection cannot re-drop.
        if current is not None:
            try:
                current._release()
            except Exception:
                pass
        for certificate in certificates:
            try:
                certificate._release()
            except Exception:
                pass
        for index in range(transferred, len(handles)):
            try:
                handles[index].__exit__(None, None, None)
            except Exception:
                pass
        raise
    return certificates


class _SSLContext(_ssl.SSLContext, metaclass=_ssl._ImmutableType):
    _immutable_type = True
    __module__ = '_ssl'
    _raw_message_enums = True

    protocol = property(lambda self: int(_ssl.SSLContext.protocol.fget(self)))
    minimum_version = property(
        lambda self: int(_ssl.SSLContext.minimum_version.fget(self)),
        lambda self, value: _ssl.SSLContext.minimum_version.fset(self, value))
    maximum_version = property(
        lambda self: int(_ssl.SSLContext.maximum_version.fget(self)),
        lambda self, value: _ssl.SSLContext.maximum_version.fset(self, value))
    verify_mode = property(
        lambda self: int(_ssl.SSLContext.verify_mode.fget(self)),
        lambda self, value: _ssl.SSLContext.verify_mode.fset(self, value))
    verify_flags = property(
        lambda self: int(_ssl.SSLContext.verify_flags.fget(self)),
        lambda self, value: _ssl.SSLContext.verify_flags.fset(self, value))
    options = property(
        lambda self: int(_ssl.SSLContext.options.fget(self)),
        lambda self, value: _ssl.SSLContext.options.fset(self, value))

    def __new__(cls, protocol, /):
        self = object.__new__(cls)
        # CPython initializes the native context in __new__, so Python
        # subclasses can call super().__new__(cls, protocol) directly.
        _ssl.SSLContext.__init__(self, protocol)
        self._private_host_flags = HOSTFLAG_NO_PARTIAL_WILDCARDS
        self._private_alpn_wire = None
        return self

    def __init__(self, protocol, /):
        pass

    @property
    def _host_flags(self):
        return self._private_host_flags

    @_host_flags.setter
    def _host_flags(self, value):
        flags = operator.index(value) & 0xffffffff
        if self._host_context is not None:
            _ssl._invoke(self._host_context.update_host_flags, flags)
        self._private_host_flags = flags
        self._hostname_common_name = not bool(flags & HOSTFLAG_NEVER_CHECK_SUBJECT)

    @property
    def hostname_checks_common_name(self):
        return not bool(self._host_flags & HOSTFLAG_NEVER_CHECK_SUBJECT)

    @hostname_checks_common_name.setter
    def hostname_checks_common_name(self, value):
        flags = self._host_flags
        self._host_flags = (flags & ~HOSTFLAG_NEVER_CHECK_SUBJECT if bool(value)
                            else flags | HOSTFLAG_NEVER_CHECK_SUBJECT)

    def _sync_policy(self):
        _ssl.SSLContext._sync_policy(self)
        if self._host_context is not None:
            _ssl._invoke(self._host_context.update_host_flags,
                         getattr(self, '_private_host_flags', HOSTFLAG_NO_PARTIAL_WILDCARDS))

    def _get_host_context(self, server_side):
        context = _ssl.SSLContext._get_host_context(self, server_side)
        _ssl._invoke(context.update_host_flags, self._host_flags)
        if self._private_alpn_wire is not None:
            _ssl._invoke(context.update_alpn_wire, self._private_alpn_wire)
        return context

    def _set_alpn_protocols(self, protos, /):
        view = _ssl._readable_buffer(protos)
        if len(view) > _ssl._LIMIT:
            raise ValueError('ALPN protocol list limit exceeded')
        wire = bytes(view)
        offset = 0
        while offset < len(wire):
            size = wire[offset]
            offset += size + 1
            if size == 0 or offset > len(wire):
                # CPython reports SSL_CTX_set_alpn_protos rejection as OOM.
                raise MemoryError()
        if self._host_context is not None:
            _ssl._invoke(self._host_context.update_alpn_wire, wire)
        self._private_alpn_wire = wire

    def set_alpn_protocols(self, protocols):
        _ssl.SSLContext.set_alpn_protocols(self, protocols)
        self._private_alpn_wire = None

    def _wrap_bio(self, incoming, outgoing, server_side, server_hostname=None,
                  *, owner=None, session=None):
        try:
            if not isinstance(incoming, MemoryBIO) or not isinstance(outgoing, MemoryBIO):
                raise TypeError('incoming and outgoing must be MemoryBIO instances')
            server_side = bool(server_side)
            if server_hostname is not None:
                if not isinstance(server_hostname, str):
                    raise TypeError('server_hostname must be str')
                str.encode(server_hostname, 'ascii')
                if '\x00' in server_hostname:
                    raise TypeError('embedded null character')
                if not server_hostname or server_hostname.startswith('.'):
                    raise ValueError('server_hostname cannot be empty or start with a dot')
            if (self.protocol == _ssl.PROTOCOL_TLS_CLIENT and server_side or
                    self.protocol == _ssl.PROTOCOL_TLS_SERVER and not server_side):
                raise SSLError('context protocol does not match server_side')
            if incoming is outgoing or incoming._owner is not None or outgoing._owner is not None:
                raise ValueError('BIO buffers must be distinct and unused')
            if session is not None:
                if type(session) is not SSLSession:
                    raise TypeError('session must be an SSLSession')
                if session._context is not self:
                    raise ValueError('Session refers to a different SSLContext')
            owner_ref = None if owner is None else weakref.ref(owner)
            context = self._get_host_context(server_side)
            connection = _ssl._invoke(context.wrap_private_bio, server_side, server_hostname)
            engine = None
            try:
                engine = object.__new__(_ssl.SSLObject)
                if session is not None:
                    _ssl._invoke(connection.set_session, session._handle)
                _ssl.SSLObject._initialize(engine, self, incoming, outgoing, connection,
                                          server_side, server_hostname)
                peer = _SSLSocket._create(engine, owner_ref)
            except BaseException:
                connection.__exit__(None, None, None)
                if engine is not None:
                    engine._connection = None
                incoming._owner = outgoing._owner = None
                raise
            return peer
        finally:
            incoming = outgoing = server_side = server_hostname = owner = session = None

    def wrap_bio(self, incoming, outgoing, server_side=False, server_hostname=None, session=None):
        # Public helpers inherited from the adapter return its public object;
        # they must not confuse a private wrapper with a WIT connection.
        peer = self._wrap_bio(incoming, outgoing, server_side,
                              self._encode_hostname(server_hostname), session=session)
        engine = peer._engine
        engine._sni_owner = None
        return engine

    def _wrap_socket(self, sock, server_side, server_hostname=None, *, owner=None, session=None):
        try:
            if not isinstance(sock, _ssl._socket.socket):
                raise TypeError('sock must be a socket')
            peer = self._wrap_bio(MemoryBIO(), MemoryBIO(), server_side, server_hostname,
                                  owner=owner, session=session)
            try:
                peer._driver = _SocketTransport(peer._engine, sock)
                if owner is None:
                    peer._engine._sni_owner = peer._driver._socket
            except BaseException:
                peer._engine.close()
                raise
            return peer
        finally:
            sock = server_side = server_hostname = owner = session = None

    def wrap_socket(self, *args, **kwargs):
        raise NotImplementedError('use _wrap_socket for private borrowed socket transport')


_MISSING = object()


class _SSLSocket(metaclass=_ssl._ImmutableType):
    _immutable_type = True
    __slots__ = ('_engine', '_owner', '_driver', '__weakref__')
    __module__ = '_ssl'

    def __new__(cls, *args, **kwargs):
        raise TypeError("cannot create '_ssl._SSLSocket' instances")

    def __init_subclass__(cls, **kwargs):
        raise TypeError("type '_ssl._SSLSocket' is not an acceptable base type")

    @classmethod
    def _create(cls, engine, owner_ref):
        self = object.__new__(cls)
        self._engine, self._owner = engine, owner_ref
        self._driver = None
        engine._sni_owner = owner_ref if owner_ref is not None else weakref.ref(self)
        return self

    @property
    def owner(self):
        return None if self._owner is None else self._owner()

    @owner.setter
    def owner(self, value):
        try:
            self._owner = None
            self._engine._sni_owner = (weakref.ref(self) if self._driver is None
                                       else self._driver._socket)
            self._owner = weakref.ref(value)
            self._engine._sni_owner = self._owner
        finally:
            value = None

    @property
    def context(self):
        return self._engine.context

    @context.setter
    def context(self, value):
        self._engine.context = value

    server_side = property(lambda self: self._engine.server_side)
    server_hostname = property(lambda self: self._engine.server_hostname)
    session_reused = property(lambda self: self._engine.session_reused)

    @property
    def session(self):
        return self._engine.session

    @session.setter
    def session(self, value):
        self._engine.session = value

    def do_handshake(self):
        if self._driver is not None:
            return self._driver._run('do_handshake')
        return self._engine.do_handshake()

    def read(self, size, buffer=_MISSING, /):
        size = operator.index(size)
        if not -sys.maxsize - 1 <= size <= sys.maxsize:
            raise OverflowError('Python int too large to convert to C ssize_t')
        target = self._engine if self._driver is None else self._driver
        if buffer is _MISSING:
            return target.read(size)
        if buffer is None:
            raise TypeError('buffer must be a writable buffer')
        return target.read(size, buffer)

    def write(self, b, /):
        return (self._engine if self._driver is None else self._driver).write(b)

    def shutdown(self):
        if self._driver is not None:
            raw = self._driver._raw_socket()
            try:
                self._driver._run('unwrap')
                return raw
            finally:
                # A retained failure traceback must not own the caller's FD.
                raw = None
        return self._engine.unwrap()

    def pending(self):
        return self._engine.pending()

    def getpeercert(self, der=False, /):
        return self._engine.getpeercert(bool(der))

    def get_channel_binding(self, cb_type='tls-unique'):
        if not isinstance(cb_type, str):
            raise TypeError('cb_type must be str')
        if '\x00' in cb_type:
            raise ValueError('embedded null character')
        return self._engine.get_channel_binding(cb_type)

    def cipher(self):
        return self._engine.cipher()

    def shared_ciphers(self):
        return self._engine.shared_ciphers()

    def version(self):
        return self._engine.version()

    def selected_alpn_protocol(self):
        return self._engine.selected_alpn_protocol()

    def compression(self):
        return self._engine.compression()

    def verify_client_post_handshake(self):
        return self._engine.verify_client_post_handshake()

    def get_verified_chain(self):
        return self._engine.get_verified_chain()

    def get_unverified_chain(self):
        return self._engine.get_unverified_chain()


class _SocketTransport:
    """Drive the shared TLS loop without taking ownership of the raw socket."""
    _run = _ssl.SSLSocket._run
    _flush_output = _ssl.SSLSocket._flush_output
    _receive_record_bytes = _ssl.SSLSocket._receive_record_bytes
    _wait_io = _ssl.SSLSocket._wait_io
    _check_message_callback = _ssl.SSLSocket._check_message_callback

    def __init__(self, engine, sock):
        try:
            self._socket = weakref.ref(sock)
            self._sslobj = engine
            self._incoming, self._outgoing = engine._incoming, engine._outgoing
            self._record_header = bytearray()
            self._record_remaining = 5
            self._reading_header = True
        finally:
            sock = None

    def _raw_socket(self):
        raw = self._socket()
        if raw is None:
            raise SSLError(0, 'Underlying socket connection gone')
        return raw

    def fileno(self):
        return _ssl._socket.socket.fileno(self._raw_socket())

    def gettimeout(self):
        return _ssl._socket.socket.gettimeout(self._raw_socket())

    def _check_tls(self):
        self._raw_socket()

    def read(self, size, buffer=None):
        size, view = _ssl._prepare_tls_read(size, buffer)
        if size == 0:
            self._raw_socket()
            return self._sslobj.read(size, view)
        # The private API propagates ragged EOF rather than suppressing it.
        return self._run('read', size, view)

    def write(self, data):
        view = _ssl._readable_buffer(data)[:_ssl._LIMIT]
        return self._run('write', view, accepted_write=True)
