"""Python 3.12-style TLS adapter over host BIO TLS and guest WASI sockets.

Import this module before dependencies and call install() to provide `ssl`.
Import, context construction and ordinary settings make no host calls.
Credential loading uses host OpenSSL after component instantiation.
"""
import enum
import encodings.idna
import operator
import os
import sys
import errno
import socket as _socket_module
from socket import SOCK_STREAM, SOL_SOCKET, SO_TYPE, create_connection, socket, error as socket_error
import _socket
import time
import select
import base64
import calendar
import warnings
import weakref
from collections import namedtuple
import _strptime  # Freeze the dependency used by time.strptime in the component.
from wit_world.imports import ssl_bio as _host
from wit_world import Err as _WitError

_LIMIT = 1024 * 1024
CHANNEL_BINDING_TYPES = ["tls-unique"]
PEM_HEADER = '-----BEGIN CERTIFICATE-----'
PEM_FOOTER = '-----END CERTIFICATE-----'
class AlertDescription(enum.IntEnum):
    ALERT_DESCRIPTION_CLOSE_NOTIFY = 0
    ALERT_DESCRIPTION_UNEXPECTED_MESSAGE = 10
    ALERT_DESCRIPTION_BAD_RECORD_MAC = 20
    ALERT_DESCRIPTION_RECORD_OVERFLOW = 22
    ALERT_DESCRIPTION_DECOMPRESSION_FAILURE = 30
    ALERT_DESCRIPTION_HANDSHAKE_FAILURE = 40
    ALERT_DESCRIPTION_BAD_CERTIFICATE = 42
    ALERT_DESCRIPTION_UNSUPPORTED_CERTIFICATE = 43
    ALERT_DESCRIPTION_CERTIFICATE_REVOKED = 44
    ALERT_DESCRIPTION_CERTIFICATE_EXPIRED = 45
    ALERT_DESCRIPTION_CERTIFICATE_UNKNOWN = 46
    ALERT_DESCRIPTION_ILLEGAL_PARAMETER = 47
    ALERT_DESCRIPTION_UNKNOWN_CA = 48
    ALERT_DESCRIPTION_ACCESS_DENIED = 49
    ALERT_DESCRIPTION_DECODE_ERROR = 50
    ALERT_DESCRIPTION_DECRYPT_ERROR = 51
    ALERT_DESCRIPTION_PROTOCOL_VERSION = 70
    ALERT_DESCRIPTION_INSUFFICIENT_SECURITY = 71
    ALERT_DESCRIPTION_INTERNAL_ERROR = 80
    ALERT_DESCRIPTION_USER_CANCELLED = 90
    ALERT_DESCRIPTION_NO_RENEGOTIATION = 100
    ALERT_DESCRIPTION_UNSUPPORTED_EXTENSION = 110
    ALERT_DESCRIPTION_CERTIFICATE_UNOBTAINABLE = 111
    ALERT_DESCRIPTION_UNRECOGNIZED_NAME = 112
    ALERT_DESCRIPTION_BAD_CERTIFICATE_STATUS_RESPONSE = 113
    ALERT_DESCRIPTION_BAD_CERTIFICATE_HASH_VALUE = 114
    ALERT_DESCRIPTION_UNKNOWN_PSK_IDENTITY = 115

globals().update(AlertDescription.__members__)
class VerifyMode(enum.IntEnum):
    CERT_NONE, CERT_OPTIONAL, CERT_REQUIRED = 0, 1, 2


CERT_NONE, CERT_OPTIONAL, CERT_REQUIRED = VerifyMode
class _SSLMethod(enum.IntEnum):
    PROTOCOL_TLS = 2
    PROTOCOL_TLSv1 = 3
    PROTOCOL_TLSv1_1 = 4
    PROTOCOL_TLSv1_2 = 5
    PROTOCOL_TLS_CLIENT = 16
    PROTOCOL_TLS_SERVER = 17


globals().update(_SSLMethod.__members__)
PROTOCOL_SSLv23 = _SSLMethod.PROTOCOL_SSLv23 = PROTOCOL_TLS
_PROTOCOL_NAMES = {value: name for name, value in _SSLMethod.__members__.items()}


class SSLErrorNumber(enum.IntEnum):
    SSL_ERROR_SSL = 1
    SSL_ERROR_WANT_READ = 2
    SSL_ERROR_WANT_WRITE = 3
    SSL_ERROR_WANT_X509_LOOKUP = 4
    SSL_ERROR_SYSCALL = 5
    SSL_ERROR_ZERO_RETURN = 6
    SSL_ERROR_WANT_CONNECT = 7
    SSL_ERROR_EOF = 8
    SSL_ERROR_INVALID_ERROR_CODE = 10


globals().update(SSLErrorNumber.__members__)


class Options(enum.IntFlag):
    # CPython's enum conversion sorts by (value, name), including aliases.
    OP_NO_SSLv2 = 0
    OP_SINGLE_DH_USE = OP_SINGLE_ECDH_USE = 0
    OP_LEGACY_SERVER_CONNECT = 0x4
    OP_ENABLE_KTLS = 0x8
    OP_IGNORE_UNEXPECTED_EOF = 0x80
    OP_NO_TICKET = 0x00004000
    OP_NO_COMPRESSION = 0x00020000
    OP_ENABLE_MIDDLEBOX_COMPAT = 0x00100000
    OP_CIPHER_SERVER_PREFERENCE = 0x00400000
    OP_NO_SSLv3 = 0x02000000
    OP_NO_TLSv1 = 0x04000000
    OP_NO_TLSv1_2 = 0x08000000
    OP_NO_TLSv1_1 = 0x10000000
    OP_NO_TLSv1_3 = 0x20000000
    OP_NO_RENEGOTIATION = 0x40000000
    OP_ALL = 0x80000050


class VerifyFlags(enum.IntFlag):
    VERIFY_DEFAULT = 0
    VERIFY_CRL_CHECK_LEAF = 4
    VERIFY_CRL_CHECK_CHAIN = 12
    VERIFY_X509_STRICT = 32
    VERIFY_ALLOW_PROXY_CERTS = 64
    VERIFY_X509_TRUSTED_FIRST = 32768
    VERIFY_X509_PARTIAL_CHAIN = 524288


for _enum in (Options, VerifyFlags):
    globals().update(_enum.__members__)
HAS_ALPN = HAS_SNI = HAS_ECDH = HAS_NEVER_CHECK_COMMON_NAME = True
HAS_SSLv2 = HAS_NPN = False
_PROTOCOL_FLAGS = {'HAS_SSLv3': 0x300, 'HAS_TLSv1': 0x301, 'HAS_TLSv1_1': 0x302,
                   'HAS_TLSv1_2': 0x303, 'HAS_TLSv1_3': 0x304}
_protocol_versions = None


def _available_versions():
    global _protocol_versions
    if _protocol_versions is None:
        _protocol_versions = tuple(_host.protocol_capabilities())
        globals().update({name: value in _protocol_versions for name, value in _PROTOCOL_FLAGS.items()})
    return _protocol_versions


class TLSVersion(enum.IntEnum):
    MINIMUM_SUPPORTED = -2
    SSLv3 = 0x300
    TLSv1 = 0x301
    TLSv1_1 = 0x302
    TLSv1_2 = 0x303
    TLSv1_3 = 0x304
    MAXIMUM_SUPPORTED = -1


class _TLSContentType(enum.IntEnum):
    CHANGE_CIPHER_SPEC = 20
    ALERT = 21
    HANDSHAKE = 22
    APPLICATION_DATA = 23
    HEADER = 0x100
    INNER_CONTENT_TYPE = 0x101


class _TLSAlertType(enum.IntEnum):
    CLOSE_NOTIFY = 0
    UNEXPECTED_MESSAGE = 10
    BAD_RECORD_MAC = 20
    DECRYPTION_FAILED = 21
    RECORD_OVERFLOW = 22
    DECOMPRESSION_FAILURE = 30
    HANDSHAKE_FAILURE = 40
    NO_CERTIFICATE = 41
    BAD_CERTIFICATE = 42
    UNSUPPORTED_CERTIFICATE = 43
    CERTIFICATE_REVOKED = 44
    CERTIFICATE_EXPIRED = 45
    CERTIFICATE_UNKNOWN = 46
    ILLEGAL_PARAMETER = 47
    UNKNOWN_CA = 48
    ACCESS_DENIED = 49
    DECODE_ERROR = 50
    DECRYPT_ERROR = 51
    EXPORT_RESTRICTION = 60
    PROTOCOL_VERSION = 70
    INSUFFICIENT_SECURITY = 71
    INTERNAL_ERROR = 80
    INAPPROPRIATE_FALLBACK = 86
    USER_CANCELED = 90
    NO_RENEGOTIATION = 100
    MISSING_EXTENSION = 109
    UNSUPPORTED_EXTENSION = 110
    CERTIFICATE_UNOBTAINABLE = 111
    UNRECOGNIZED_NAME = 112
    BAD_CERTIFICATE_STATUS_RESPONSE = 113
    BAD_CERTIFICATE_HASH_VALUE = 114
    UNKNOWN_PSK_IDENTITY = 115
    CERTIFICATE_REQUIRED = 116
    NO_APPLICATION_PROTOCOL = 120


class _TLSMessageType(enum.IntEnum):
    HELLO_REQUEST = 0
    CLIENT_HELLO = 1
    SERVER_HELLO = 2
    HELLO_VERIFY_REQUEST = 3
    NEWSESSION_TICKET = 4
    END_OF_EARLY_DATA = 5
    HELLO_RETRY_REQUEST = 6
    ENCRYPTED_EXTENSIONS = 8
    CERTIFICATE = 11
    SERVER_KEY_EXCHANGE = 12
    CERTIFICATE_REQUEST = 13
    SERVER_DONE = 14
    CERTIFICATE_VERIFY = 15
    CLIENT_KEY_EXCHANGE = 16
    FINISHED = 20
    CERTIFICATE_URL = 21
    CERTIFICATE_STATUS = 22
    SUPPLEMENTAL_DATA = 23
    KEY_UPDATE = 24
    NEXT_PROTO = 67
    MESSAGE_HASH = 254
    CHANGE_CIPHER_SPEC = 0x101


def _enum_or_int(cls, value):
    try:
        return cls(value)
    except ValueError:
        return value


_PURPOSE_OIDS = {
    "1.3.6.1.5.5.7.3.1": (129, "serverAuth", "TLS Web Server Authentication", "1.3.6.1.5.5.7.3.1"),
    "1.3.6.1.5.5.7.3.2": (130, "clientAuth", "TLS Web Client Authentication", "1.3.6.1.5.5.7.3.2"),
}


def _asn1_text(text, numeric_only):
    if not isinstance(text, str):
        raise TypeError("OID lookup argument must be str")
    if '\x00' in text:
        raise ValueError("embedded null character")
    # Known purpose values must be constructible during componentization.
    if text in _PURPOSE_OIDS:
        return _PURPOSE_OIDS[text]
    query = _host.Asn1Query_Oid(text) if numeric_only else _host.Asn1Query_Name(text)
    try:
        obj = _invoke(_host.lookup_asn1_object, query)
    except ValueError as error:
        if str(error) != "Unknown object":
            raise
        raise ValueError(f"unknown object {text!r}") from None
    return obj.nid, obj.shortname, obj.longname, obj.oid


class _ASN1Object(namedtuple("_ASN1Object", "nid shortname longname oid")):
    __slots__ = ()

    def __new__(cls, oid):
        return super().__new__(cls, *_asn1_text(oid, True))

    @classmethod
    def fromname(cls, name):
        return super().__new__(cls, *_asn1_text(name, False))

    @classmethod
    def fromnid(cls, nid):
        nid = _signed_int(nid)
        obj = _invoke(_host.lookup_asn1_object, _host.Asn1Query_Nid(nid))
        return super().__new__(cls, obj.nid, obj.shortname, obj.longname, obj.oid)


class Purpose(_ASN1Object, enum.Enum):
    SERVER_AUTH = "1.3.6.1.5.5.7.3.1"
    CLIENT_AUTH = "1.3.6.1.5.5.7.3.2"


class _ImmutableType(type):
    def __setattr__(cls, name, value):
        if cls.__dict__.get('_immutable_type', False):
            raise TypeError("cannot set attributes of immutable type '%s'" % cls.__name__)
        return super().__setattr__(name, value)

    def __delattr__(cls, name):
        if cls.__dict__.get('_immutable_type', False):
            raise TypeError("cannot delete attributes of immutable type '%s'" % cls.__name__)
        return super().__delattr__(name)


class SSLError(OSError, metaclass=_ImmutableType):
    _immutable_type = True
    def __str__(self):
        return self.strerror if isinstance(self.strerror, str) else str(self.args)


class SSLWantReadError(SSLError):
    pass


class SSLWantWriteError(SSLError):
    pass


class SSLZeroReturnError(SSLError):
    pass


class SSLSyscallError(SSLError):
    pass


class SSLEOFError(SSLError):
    pass


class SSLCertVerificationError(SSLError):
    pass


CertificateError = SSLCertVerificationError


def __getattr__(name):
    if name == '_OPENSSL_API_VERSION':
        import tarantool__ssl
        return tarantool__ssl._OPENSSL_API_VERSION
    if name in _PROTOCOL_FLAGS:
        _available_versions()
        return globals()[name]
    if name not in ("OPENSSL_VERSION", "OPENSSL_VERSION_NUMBER", "OPENSSL_VERSION_INFO"):
        raise AttributeError("module 'ssl' has no attribute " + repr(name))
    # componentize-py executes imports before host WIT functions are available.
    # Query only on runtime access, then cache the immutable host backend facts.
    backend = _host.backend()
    number = backend.number
    values = {
        "OPENSSL_VERSION": backend.version,
        "OPENSSL_VERSION_NUMBER": number,
        "OPENSSL_VERSION_INFO": ((number >> 28) & 15, (number >> 20) & 255,
                                 (number >> 12) & 255, (number >> 4) & 255, number & 15),
    }
    globals().update(values)
    return values[name]


def __dir__():
    return sorted(set(globals()) | set(_PROTOCOL_FLAGS) | {"OPENSSL_VERSION", "OPENSSL_VERSION_NUMBER", "OPENSSL_VERSION_INFO"})


def _invoke(function, *args):
    try:
        return function(*args)
    except _WitError as caught:
        error = caught.value
    # Raise outside the handler: the frozen generated WIT exception must not
    # become __context__. Python tools such as unittest edit chained tracebacks.
    kind = error.kind
    if kind == _host.ErrorKind.INVALID_INPUT:
        raise ValueError(error.message) from None
    details = _host.describe_error(error.message)
    message = error.message
    if details.library and details.reason:
        message = '[%s: %s] %s' % (details.library, details.reason, message)
    if kind == _host.ErrorKind.WANT_READ:
        exc = SSLWantReadError(SSL_ERROR_WANT_READ, message)
    elif kind == _host.ErrorKind.WANT_WRITE:
        exc = SSLWantWriteError(SSL_ERROR_WANT_WRITE, message)
    elif kind == _host.ErrorKind.ZERO_RETURN:
        exc = SSLZeroReturnError(SSL_ERROR_ZERO_RETURN, message)
    elif kind == _host.ErrorKind.EOF:
        exc = SSLEOFError(SSL_ERROR_EOF, message)
    elif kind == _host.ErrorKind.VERIFICATION:
        exc = SSLCertVerificationError(SSL_ERROR_SSL, message)
        exc.verify_code = error.verify_code
        exc.verify_message = (_host.verification_message(error.verify_code)
                              if error.verify_code is not None else
                              'unknown certificate verification error')
    else:
        exc = SSLError(SSL_ERROR_SSL, message)
    exc.library, exc.reason = details.library, details.reason
    raise exc from None


def _signed_int(value):
    value = operator.index(value)
    if not -(1 << 31) <= value < (1 << 31):
        raise OverflowError("Python int too large to convert to C int")
    return value


def get_protocol_name(protocol_code):
    return _PROTOCOL_NAMES.get(protocol_code, "<unknown>")


def RAND_bytes(num, /):
    num = _signed_int(num)
    if num < 0:
        raise ValueError("num must be positive")
    return bytes(_invoke(_host.random_bytes, num))


def RAND_status():
    return _host.random_status()


def RAND_add(bytes, entropy, /):
    if isinstance(bytes, str):
        data = bytes.encode('utf-8')
    else:
        view = memoryview(bytes)
        if not view.c_contiguous:
            raise BufferError("underlying buffer is not C-contiguous")
        if view.nbytes > _LIMIT:
            raise ValueError("random byte transfer exceeds 1 MiB")
        data = view.tobytes()
    if len(data) > _LIMIT:
        raise ValueError("random byte transfer exceeds 1 MiB")
    if isinstance(entropy, float):
        entropy = float.__float__(entropy)
    elif any('__float__' in base.__dict__ or '__index__' in base.__dict__
             for base in type(entropy).__mro__):
        entropy = float(entropy)
    else:
        raise TypeError("entropy must be a real number")
    _invoke(_host.random_add, data, entropy)


DefaultVerifyPaths = namedtuple("DefaultVerifyPaths", "cafile capath openssl_cafile_env openssl_cafile openssl_capath_env openssl_capath")


def get_default_verify_paths():
    defaults = _invoke(_host.default_verify_paths)
    parts = (defaults.cafile_env, os.fsdecode(bytes(defaults.cafile)),
             defaults.capath_env, os.fsdecode(bytes(defaults.capath)))
    cafile = os.environ.get(parts[0], parts[1])
    capath = os.environ.get(parts[2], parts[3])
    return DefaultVerifyPaths(cafile if os.path.isfile(cafile) else None,
                              capath if os.path.isdir(capath) else None, *parts)


class MemoryBIO(metaclass=_ImmutableType):
    _immutable_type = True

    def __init_subclass__(cls, **kwargs):
        raise TypeError("MemoryBIO is not an acceptable base type")

    def __init__(self):
        self._buffer = bytearray()
        self._written_eof = False
        self._owner = None

    @property
    def pending(self):
        return len(self._buffer)

    @property
    def eof(self):
        return self._written_eof and not self._buffer

    def read(self, n=-1):
        n = _signed_int(n)
        if n < 0:
            n = len(self._buffer)
        data = bytes(self._buffer[:n])
        del self._buffer[:n]
        return data

    def write(self, buf):
        view = _readable_buffer(buf)
        if self._written_eof:
            raise SSLError(SSL_ERROR_SSL, "cannot write() after write_eof()")
        self._buffer.extend(view)
        return view.nbytes

    def write_eof(self):
        self._written_eof = True


class SSLContext:
    sslobject_class = None
    sslsocket_class = None
    __slots__ = ("_protocol", "_verify_mode", "_check_hostname", "_minimum_version",
                 "_maximum_version", "_default_certs", "_ca", "_cert", "_key", "_alpn",
        "_options", "_verify_flags", "_ciphers", "_hostname_common_name", "_security_level", "_suiteb_flags",
                 "_cache_key", "_host_context", "_dh", "_dh_security_level", "_curve", "_sni_callback", "_credential_history",
                 "_post_handshake_auth", "_num_tickets", "_keylog_filename", "_keylog_file",
                 "_capaths", "_store_snapshot_key", "_policy_snapshot_key", "_settings_snapshot_key", "_tls13_snapshot_key",
                 "_message_callback", "__dict__", "__weakref__")
    def __init__(self, protocol=PROTOCOL_TLS):
        protocol = _signed_int(protocol)
        if protocol not in (PROTOCOL_TLS_CLIENT, PROTOCOL_TLS_SERVER, PROTOCOL_TLS, PROTOCOL_TLSv1, PROTOCOL_TLSv1_1, PROTOCOL_TLSv1_2):
            raise ValueError("invalid or unsupported protocol version")
        if protocol in (PROTOCOL_TLS, PROTOCOL_TLSv1, PROTOCOL_TLSv1_1, PROTOCOL_TLSv1_2):
            warnings.warn("ssl.%s is deprecated" % _SSLMethod(protocol).name, DeprecationWarning, stacklevel=2)
        self._protocol = _SSLMethod(protocol)
        self._verify_mode = CERT_REQUIRED if protocol == PROTOCOL_TLS_CLIENT else CERT_NONE
        self._check_hostname = protocol == PROTOCOL_TLS_CLIENT
        self._minimum_version = TLSVersion.MINIMUM_SUPPORTED if protocol in (PROTOCOL_TLSv1, PROTOCOL_TLSv1_1, PROTOCOL_TLSv1_2) else TLSVersion.TLSv1_2
        self._maximum_version = TLSVersion.MAXIMUM_SUPPORTED
        self._default_certs = False
        self._default_sources = (False, False)
        self._ca = None
        self._capaths = []
        self._store_snapshot_key = None
        self._policy_snapshot_key = None
        self._settings_snapshot_key = None
        self._tls13_snapshot_key = None
        self._cert = None
        self._key = None
        self._alpn = []
        self._options = OP_ALL | OP_NO_SSLv3 | OP_NO_COMPRESSION | OP_CIPHER_SERVER_PREFERENCE | OP_ENABLE_MIDDLEBOX_COMPAT
        self._verify_flags = VERIFY_X509_TRUSTED_FIRST
        self._ciphers = None
        self._security_level = 2
        self._suiteb_flags = 0
        self._hostname_common_name = True
        self._cache_key = None
        self._host_context = None
        self._dh = None
        self._dh_security_level = 2
        self._credential_history = []
        self._curve = None
        self._sni_callback = None
        self._post_handshake_auth = False
        self._num_tickets = 2
        self._keylog_filename = None
        self._keylog_file = None
        self._message_callback = None

    @property
    def _msg_callback(self):
        return self._message_callback

    @_msg_callback.setter
    def _msg_callback(self, callback):
        if callback is not None and not hasattr(callback, '__call__'):
            raise TypeError(f"{callback} is not callable.")
        self._message_callback = callback

    def __del__(self):
        try:
            context = getattr(self, "_host_context", None)
            if context is not None:
                context.__exit__(None, None, None)
                self._host_context = None
        except Exception:
            pass

        try:
            file = getattr(self, '_keylog_file', None)
            if file is not None:
                file.close()
        except Exception:
            pass

    @property
    def keylog_filename(self):
        return self._keylog_filename

    @keylog_filename.setter
    def keylog_filename(self, filename):
        old_file = self._keylog_file
        self._keylog_file = self._keylog_filename = None
        if old_file is not None:
            old_file.close()
        if filename is None:
            return
        file = open(os.fspath(filename), 'ab')
        try:
            try:
                empty = file.tell() == 0
            except OSError:
                empty = False
            if empty:
                file.write(b'# TLS secrets log file, generated by OpenSSL / Python\n')
                file.flush()
            self._keylog_file = file
            self._keylog_filename = filename
        except BaseException:
            file.close()
            raise

    @property
    def protocol(self):
        return self._protocol

    @property
    def minimum_version(self):
        return self._minimum_version

    @minimum_version.setter
    def minimum_version(self, value):
        if value == TLSVersion.SSLv3:
            self.options &= ~OP_NO_SSLv3
        bound = self._version_bound(value)
        self._set_setting_value('_minimum_version', TLSVersion(max(_available_versions())) if bound == TLSVersion.MAXIMUM_SUPPORTED else bound)

    @property
    def maximum_version(self):
        return self._maximum_version

    @maximum_version.setter
    def maximum_version(self, value):
        bound = self._version_bound(value)
        self._set_setting_value('_maximum_version', TLSVersion(min(_available_versions())) if bound == TLSVersion.MINIMUM_SUPPORTED else bound)

    def _version_bound(self, value):
        number = operator.index(value)
        if not -(1 << 63) <= number < (1 << 63) or number > 0x7fffffff:
            raise OverflowError("TLS version exceeds native integer range")
        if self.protocol in (PROTOCOL_TLSv1, PROTOCOL_TLSv1_1, PROTOCOL_TLSv1_2):
            raise ValueError("this protocol does not support modifying version bounds")
        value = TLSVersion(number)
        if value in (TLSVersion.SSLv3, TLSVersion.TLSv1, TLSVersion.TLSv1_1):
            warnings.warn("ssl.TLSVersion.%s is deprecated" % TLSVersion(value).name, DeprecationWarning, stacklevel=3)
        return value

    @property
    def post_handshake_auth(self):
        return self._post_handshake_auth

    @post_handshake_auth.setter
    def post_handshake_auth(self, value):
        self._set_tls13_value('_post_handshake_auth', bool(value))

    @property
    def num_tickets(self):
        return self._num_tickets

    @num_tickets.setter
    def num_tickets(self, value):
        value = operator.index(value)
        if not -(1 << 63) <= value < (1 << 63):
            raise OverflowError("num_tickets exceeds signed long")
        if value < 0:
            raise ValueError("value must be non-negative")
        if self.protocol != PROTOCOL_TLS_SERVER:
            raise ValueError("SSLContext is not a server context")
        self._set_tls13_value('_num_tickets', value)

    def _set_tls13_value(self, name, value):
        previous = getattr(self, name)
        setattr(self, name, value)
        try:
            self._sync_tls13()
        except BaseException:
            setattr(self, name, previous)
            raise

    def _tls13_key(self):
        return self._post_handshake_auth, self._num_tickets

    def _sync_tls13(self):
        if self._host_context is not None and self._tls13_key() != self._tls13_snapshot_key:
            _invoke(self._host_context.update_tls13,
                    _host.Tls13Settings(post_handshake_auth=self._post_handshake_auth,
                                        num_tickets=self._num_tickets))
            self._tls13_snapshot_key = self._tls13_key()

    @property
    def verify_mode(self):
        return VerifyMode(self._verify_mode)

    @property
    def options(self):
        return Options(self._options)

    @options.setter
    def options(self, value):
        value = _unsigned_mask(value)
        deprecated = OP_NO_SSLv3 | OP_NO_TLSv1 | OP_NO_TLSv1_1 | OP_NO_TLSv1_2 | OP_NO_TLSv1_3
        if value & ~self._options & deprecated:
            warnings.warn("ssl.OP_NO_SSL*/ssl.OP_NO_TLS* options are deprecated", DeprecationWarning, stacklevel=2)
        self._set_policy_value('_options', value)

    @property
    def verify_flags(self):
        return VerifyFlags(self._verify_flags)

    @verify_flags.setter
    def verify_flags(self, value):
        # CPython's unsigned-long-mask conversion wraps unlike options.
        mask = operator.index(value) & 0xffffffffffffffff
        # OpenSSL set_flags adds POLICY_CHECK when any newly set policy bit
        # is present. CPython sends only the set/clear delta: repeating an
        # EXPLICIT_POLICY-only assignment can therefore clear POLICY_CHECK.
        if (mask & ~self._verify_flags) & 0x780:
            mask |= 0x80
        self._set_policy_value('_verify_flags', mask)

    @property
    def hostname_checks_common_name(self):
        return self._hostname_common_name

    @hostname_checks_common_name.setter
    def hostname_checks_common_name(self, value):
        self._set_policy_value('_hostname_common_name', bool(value))

    def _set_policy_value(self, name, value):
        previous = getattr(self, name)
        setattr(self, name, value)
        try:
            self._sync_policy()
        except BaseException:
            setattr(self, name, previous)
            raise

    def _policy_key(self):
        return self._options, self._verify_flags, self._ciphers, self._hostname_common_name, self._security_level, self._suiteb_flags

    def _sync_policy(self):
        if self._host_context is not None and self._policy_key() != self._policy_snapshot_key:
            _invoke(self._host_context.update_policy, self._policy())
            self._policy_snapshot_key = self._policy_key()

    def _policy(self, ciphers=None):
        ciphers = self._ciphers if ciphers is None else ciphers
        return _host.ContextPolicy(options=self._options, verify_flags=self._verify_flags,
                                   ciphers=ciphers,
                                   hostname_common_name=self._hostname_common_name)

    def _seeded_policy(self, ciphers=None):
        return _host.SeededPolicy(policy=self._policy(ciphers),
                                  seed=_host.PolicySeed(security_level=self._security_level,
                                                        suiteb_flags=self._suiteb_flags))

    def set_ciphers(self, ciphers):
        if not isinstance(ciphers, str):
            raise TypeError("ciphers must be a string")
        ciphers = str.__str__(ciphers)
        try:
            details = _invoke(_host.inspect_seeded_policy, self._seeded_policy(ciphers))
        except SSLError:
            # CPython deliberately supplies a plain message here, ignoring
            # OpenSSL's NO_CIPHER_MATCH code and leaving symbolic attrs absent.
            raise SSLError('No cipher can be selected.') from None
        previous = self._security_level, self._suiteb_flags
        self._security_level = details.security_level
        for prefix, flags in (('SUITEB128ONLY', 0x10000), ('SUITEB128', 0x30000), ('SUITEB192', 0x20000)):
            if ciphers.startswith(prefix):
                self._suiteb_flags = flags
                break
        try:
            self._set_policy_value('_ciphers', ciphers)
        except BaseException:
            self._security_level, self._suiteb_flags = previous
            raise

    def _inspect_policy(self):
        if self._host_context is not None:
            return _invoke(self._host_context.inspect_policy)
        return _invoke(_host.inspect_seeded_policy, self._seeded_policy())

    def get_ciphers(self):
        details = self._inspect_policy()
        fields = ("id", "name", "protocol", "description", "strength_bits", "alg_bits",
                  "aead", "symmetric", "digest", "kea", "auth")
        return [{field: getattr(cipher, field) for field in fields} for cipher in details.ciphers]

    @property
    def security_level(self):
        return self._inspect_policy().security_level

    @verify_mode.setter
    def verify_mode(self, value):
        value = operator.index(value)
        if value not in (CERT_NONE, CERT_OPTIONAL, CERT_REQUIRED):
            raise ValueError("invalid verify_mode")
        if value == CERT_NONE and self.check_hostname:
            raise ValueError("Cannot set verify_mode to CERT_NONE when check_hostname is enabled")
        self._set_setting_value('_verify_mode', value)

    @property
    def check_hostname(self):
        return self._check_hostname

    @check_hostname.setter
    def check_hostname(self, value):
        value = bool(value)
        previous = self._check_hostname, self._verify_mode
        self._check_hostname = value
        if value and self._verify_mode == CERT_NONE:
            self._verify_mode = CERT_REQUIRED
        try:
            self._sync_settings()
        except BaseException:
            self._check_hostname, self._verify_mode = previous
            raise

    def _set_setting_value(self, name, value):
        previous = getattr(self, name)
        setattr(self, name, value)
        try:
            self._sync_settings()
        except BaseException:
            setattr(self, name, previous)
            raise

    def _settings_key(self):
        return self._verify_mode, self._check_hostname, self._minimum_version, self._maximum_version

    def _settings(self):
        fixed = {PROTOCOL_TLSv1: TLSVersion.TLSv1, PROTOCOL_TLSv1_1: TLSVersion.TLSv1_1,
                 PROTOCOL_TLSv1_2: TLSVersion.TLSv1_2}.get(self.protocol)
        return _host.ProtocolSettings(
            verify=(_host.VerifyMode.NONE, _host.VerifyMode.OPTIONAL, _host.VerifyMode.REQUIRED)[self._verify_mode],
            check_hostname=self._check_hostname,
            minimum=int(fixed or (0 if self._minimum_version == TLSVersion.MINIMUM_SUPPORTED else self._minimum_version)),
            maximum=int(fixed or (0 if self._maximum_version == TLSVersion.MAXIMUM_SUPPORTED else self._maximum_version)))

    def _sync_settings(self):
        if self._host_context is not None and self._settings_key() != self._settings_snapshot_key:
            _invoke(self._host_context.update_protocol_settings, self._settings())
            self._settings_snapshot_key = self._settings_key()

    def load_verify_locations(self, cafile=None, capath=None, cadata=None):
        paths = list(self._capaths)
        if capath is not None:
            try:
                path = os.fsdecode(os.fspath(capath))
            except TypeError:
                raise TypeError("capath should be a valid filesystem path") from None
            if '\x00' in path:
                raise ValueError("embedded null byte")
            if not path:
                raise SSLError(SSL_ERROR_SSL, "invalid directory")
            for part in path.split(os.pathsep):
                if part and part not in paths:
                    paths.append(part)
            if len(paths) > 16:
                raise ValueError("TLS CA directory count exceeds 16")
        if cafile is None and capath is None and cadata is None:
            raise TypeError("cafile, capath and cadata cannot be all omitted")
        bundles = []
        if cafile is not None:
            with open(cafile, "rb") as f:
                data = f.read(_LIMIT + 1)
            if not data:
                raise SSLError(SSL_ERROR_SSL, "CA file contains no certificates")
            bundles.append(_invoke(_host.prepare_store_pem, data))
        if cadata is not None:
            if isinstance(cadata, str):
                try:
                    data = _host.CertificateInput_Pem(cadata.encode("ascii"))
                except UnicodeEncodeError:
                    raise TypeError("cadata should be an ASCII string or bytes-like object") from None
            else:
                try:
                    view = memoryview(cadata)
                except TypeError:
                    raise TypeError("cadata should be an ASCII string or bytes-like object") from None
                if not view.c_contiguous:
                    raise BufferError("memoryview: underlying buffer is not C-contiguous")
                data = _host.CertificateInput_Der(view.tobytes())
            try:
                bundle = _invoke(_host.prepare_ca, data)
            except SSLError as error:
                if str(error) != "certificate input contains no certificates":
                    raise
                prefix = "no start line" if isinstance(cadata, str) else "not enough data"
                error = SSLError(0, prefix + ": cadata does not contain a certificate")
                error.library = error.reason = None
                raise error from None
            bundles.append(bundle)
        combined = (self._ca or b"") + b"".join(bundles)
        if len(combined) > _LIMIT:
            raise ValueError("TLS credential limit exceeded")
        previous = self._ca, self._capaths
        self._ca, self._capaths = combined or None, paths
        # Register paths without reading them, as OpenSSL does. Only eager
        # CA bytes change now; directory contents refresh at the next TLS I/O.
        try:
            self._sync_trust(self._store_snapshot_key[2] if self._store_snapshot_key else ())
        except BaseException:
            self._ca, self._capaths = previous
            raise

    def _directory_snapshot(self):
        directories = []
        total_bytes = total_files = 0
        for path in self._capaths:
            files = []
            try:
                with os.scandir(path) as entries:
                    for entry in entries:
                        prefix, separator, suffix = entry.name.partition('.')
                        number = suffix[1:] if suffix.startswith('r') else suffix
                        if not separator or len(prefix) != 8 or not all(c in '0123456789abcdef' for c in prefix):
                            continue
                        if not number or len(number) > 10 or not all(c in '0123456789' for c in number):
                            continue
                        index = int(number)
                        if index > 0xffffffff or number != str(index):
                            continue
                        try:
                            if not entry.is_file():
                                continue
                            with open(entry.path, 'rb') as file:
                                data = file.read(_LIMIT + 1)
                        except OSError:
                            continue
                        total_files += 1
                        total_bytes += len(entry.name) + len(data)
                        if total_files > 4096 or total_bytes > _LIMIT:
                            raise ValueError("TLS CA directory snapshot limit exceeded")
                        files.append((entry.name, data))
            except OSError:
                # OpenSSL registers even nonexistent/unreadable directories;
                # verification later fails if no permitted issuer is available.
                pass
            directories.append(tuple(sorted(files)))
        return tuple(directories)

    @staticmethod
    def _host_directories(directories):
        return [_host.TrustDirectory(files=[_host.TrustFile(name=name, data=data)
                                            for name, data in files])
                for files in directories]

    def _sync_trust(self, directories=None):
        if self._host_context is None:
            return
        if directories is None:
            directories = self._directory_snapshot()
        trust = (self._ca, self._default_certs, directories, self._default_sources)
        if trust != self._store_snapshot_key:
            _invoke(self._host_context.update_trust, self._ca, self._default_certs,
                    self._host_directories(directories))
            _invoke(self._host_context.load_default_trust,
                    _host.DefaultTrustSources(*self._default_sources))
            self._store_snapshot_key = trust

    def _inspect_store(self, query):
        if self._host_context is not None:
            return _invoke(self._host_context.inspect_store, query)
        return _invoke(_host.inspect_store_with_default_sources, self._ca,
                       _host.DefaultTrustSources(*self._default_sources), query)

    def cert_store_stats(self):
        details = self._inspect_store(_host.StoreQuery.STATS)
        return {"x509": details.x509, "x509_ca": details.x509_ca, "crl": details.crl}

    def get_ca_certs(self, binary_form=False):
        query = _host.StoreQuery.DER if binary_form else _host.StoreQuery.DECODED
        details = self._inspect_store(query)
        return [cert.value if binary_form else _certificate_dict(cert.value)
                for cert in details.certificates]

    def load_cert_chain(self, certfile, keyfile=None, password=None):
        supplied = _password_bytes(password) if password is not None and not callable(password) else None
        with open(certfile, "rb") as f:
            cert = f.read(_LIMIT + 1)
        with open(certfile if keyfile is None else keyfile, "rb") as f:
            key = f.read(_LIMIT + 1)
        prepared = _invoke(_host.prepare_policy_credentials, cert, key, supplied, self._seeded_policy())
        if isinstance(prepared, _host.PreparedPolicyCredentials_PasswordRequired):
            if not callable(password):
                raise SSLError(SSL_ERROR_SSL, "encrypted private key requires a password")
            supplied = _password_bytes(password(), callback=True)
            prepared = _invoke(_host.prepare_policy_credentials, cert, key, supplied, self._seeded_policy())
        details = prepared.value
        history = [(kind, entry) for kind, entry in self._credential_history if kind != details.key_type]
        history.append((details.key_type, _host.CredentialHistory(credentials=details.credentials,
                                                               security_level=self._security_level)))
        if len(history) > 16 or sum(len(entry.credentials.cert_pem) + len(entry.credentials.key_pem)
                                    for _, entry in history) > _LIMIT:
            raise ValueError('retained TLS credential limit exceeded')
        if self._host_context is not None:
            _invoke(self._host_context.update_credentials, details.credentials)
        self._cert, self._key = details.credentials.cert_pem, details.credentials.key_pem
        self._credential_history = history

    def load_default_certs(self, purpose=Purpose.SERVER_AUTH):
        if not isinstance(purpose, _ASN1Object):
            raise TypeError(purpose)
        self.set_default_verify_paths()

    def load_dh_params(self, dhfile, /):
        with open(os.fspath(dhfile), "rb") as file:
            data = file.read(_LIMIT + 1)
        prepared = _invoke(_host.prepare_key_exchange_seeded,
                           _host.KeyExchange(dh_pem=data, curve=None), self._seeded_policy())
        if self._host_context is not None:
            _invoke(self._host_context.update_key_exchange, _host.KeyExchangeUpdate_DhPem(prepared.dh_pem))
        self._dh = prepared.dh_pem
        self._dh_security_level = self._security_level

    def set_ecdh_curve(self, name, /):
        raw = os.fsencode(name)
        if b"\0" in raw:
            raise ValueError("embedded null byte")
        try:
            curve = raw.decode("ascii")
        except UnicodeDecodeError:
            raise ValueError("unknown elliptic curve name") from None
        _invoke(_host.prepare_key_exchange_seeded,
                _host.KeyExchange(dh_pem=None, curve=curve), self._seeded_policy())
        if self._host_context is not None:
            _invoke(self._host_context.update_key_exchange, _host.KeyExchangeUpdate_Curve(curve))
        self._curve = curve

    def set_default_verify_paths(self):
        defaults = _invoke(_host.default_verify_paths)
        file_override = defaults.cafile_env in os.environ
        dir_override = defaults.capath_env in os.environ
        previous = self._ca, self._capaths[:], self._default_sources
        try:
            if file_override:
                try:
                    with open(os.environ[defaults.cafile_env], 'rb') as source:
                        data = source.read(_LIMIT + 1)
                    prepared = _invoke(_host.prepare_store_pem, data)
                except (OSError, SSLError):
                    # Native default-path loading ignores absent/invalid files.
                    pass
                else:
                    combined = (self._ca or b'') + prepared
                    if len(combined) > _LIMIT:
                        raise ValueError('TLS trust store limit exceeded')
                    self._ca = combined
            if dir_override:
                path = os.environ[defaults.capath_env]
                # OpenSSL accepts a separator-delimited directory list.
                for directory in path.split(os.pathsep):
                    if directory and directory not in self._capaths:
                        self._capaths.append(directory)
            self._default_sources = tuple(old or selected for old, selected in
                zip(self._default_sources, (not file_override, not dir_override)))
            self._sync_trust()
        except BaseException:
            self._ca, self._capaths, self._default_sources = previous
            raise

    def set_alpn_protocols(self, protocols):
        values = []
        size = 0
        for name in protocols:
            data = bytes(name, 'ascii')
            if not 1 <= len(data) <= 255:
                raise SSLError("ALPN protocols must contain 1..255 ASCII bytes")
            size += 1 + len(data)
            if size > _LIMIT:
                raise ValueError("ALPN protocol list limit exceeded")
            values.append(data.decode('ascii'))
        if self._host_context is not None:
            _invoke(self._host_context.update_alpn, values)
        self._alpn = values

    def set_npn_protocols(self, npn_protocols):
        warnings.warn("ssl NPN is deprecated, use ALPN instead", DeprecationWarning, stacklevel=2)
        encoded = bytearray()
        for protocol in npn_protocols:
            value = bytes(protocol, 'ascii')
            if not 1 <= len(value) <= 255:
                raise SSLError('NPN protocols must be 1 to 255 in length')
            encoded.append(len(value))
            encoded.extend(value)
        # HAS_NPN is false, as in CPython 3.12 with OpenSSL. Keep the native
        # missing-hook error and permit an application's subclass hook.
        self._set_npn_protocols(encoded)

    @property
    def sni_callback(self):
        return self._sni_callback

    @sni_callback.setter
    def sni_callback(self, callback):
        if self.protocol == PROTOCOL_TLS_CLIENT:
            raise ValueError("sni_callback cannot be set on TLS_CLIENT context")
        self._sni_callback = None
        if callback is not None:
            if not callable(callback):
                if self._host_context is not None:
                    _invoke(self._host_context.update_sni, False)
                raise TypeError("not a callable object")
        if self._host_context is not None:
            _invoke(self._host_context.update_sni, callback is not None)
        self._sni_callback = callback

    def set_servername_callback(self, callback):
        if callback is None:
            self.sni_callback = None
        else:
            if not callable(callback):
                raise TypeError("not a callable object")
            def shim(obj, name, context):
                encoded = name.encode("idna").decode("ascii") if name is not None else None
                return callback(obj, encoded, context)
            self.sni_callback = shim

    def wrap_socket(self, sock, server_side=False, do_handshake_on_connect=True,
                    suppress_ragged_eofs=True, server_hostname=None, session=None):
        return self.sslsocket_class._create(sock=sock, context=self, server_side=server_side,
                    do_handshake_on_connect=do_handshake_on_connect,
                    suppress_ragged_eofs=suppress_ragged_eofs,
                    server_hostname=server_hostname, session=session)

    def _encode_hostname(self, hostname):
        if hostname is None:
            return None
        if isinstance(hostname, str):
            name = hostname.encode("idna").decode("ascii")
        else:
            name = hostname.decode("ascii")
        if '\x00' in name:
            raise TypeError("argument must be encoded string without null bytes, not str")
        if not name or name.startswith('.'):
            raise ValueError("server_hostname cannot be an empty string or start with a leading dot.")
        return name

    def wrap_bio(self, incoming, outgoing, server_side=False, server_hostname=None, session=None):
        return self.sslobject_class._create(incoming, outgoing, server_side=server_side,
                    server_hostname=self._encode_hostname(server_hostname),
                    session=session, context=self)

    def _wrap_bio(self, incoming, outgoing, server_side=False, server_hostname=None, session=None):
        if session is not None:
            if type(session) is not SSLSession:
                raise TypeError("session must be an SSLSession")
            if server_side:
                raise ValueError("session is client-only")
            if session._context is not self:
                raise ValueError("Session refers to a different SSLContext")
        if not isinstance(incoming, MemoryBIO) or not isinstance(outgoing, MemoryBIO):
            raise TypeError("incoming and outgoing must be MemoryBIO instances")
        if incoming is outgoing or incoming._owner is not None or outgoing._owner is not None:
            raise ValueError("BIO buffers must be distinct and unused")
        if self.protocol in (PROTOCOL_TLS_CLIENT, PROTOCOL_TLS_SERVER) and bool(server_side) != (self.protocol == PROTOCOL_TLS_SERVER):
            raise ValueError("context protocol does not match server_side")
        if server_side and server_hostname is not None:
            raise ValueError("server_hostname is client-only")
        if self.check_hostname and not server_hostname:
            raise ValueError("check_hostname requires server_hostname")
        name = server_hostname
        context = self._get_host_context(server_side)
        connection = _invoke(context.wrap_with_role, bool(server_side), name)
        try:
            if session is not None:
                _invoke(connection.set_session, session._handle)
        except BaseException:
            connection.__exit__(None, None, None)
            raise
        return connection

    def _get_host_context(self, server_side):
        policy = self._policy()
        directories = self._directory_snapshot()
        key = (self.protocol,)
        if key == self._cache_key:
            self._sync_tls13()
            self._sync_settings()
            self._sync_policy()
            self._sync_trust(directories)
            return self._host_context
        config = _host.Config(
            server=self.protocol == PROTOCOL_TLS_SERVER,
            verify=(_host.VerifyMode.NONE, _host.VerifyMode.OPTIONAL, _host.VerifyMode.REQUIRED)[self.verify_mode],
            check_hostname=self.check_hostname, default_certs=self._default_certs,
            ca_pem=self._ca, cert_pem=None, key_pem=None, alpn=self._alpn,
            minimum=_host.TlsVersion.TLS12, maximum=None,
        )
        context = _invoke(_host.create_context_with_history, config, self._seeded_policy(),
                          _host.ContextHistory(key_exchange=_host.KeyExchangeHistory(
                              settings=_host.KeyExchange(dh_pem=self._dh, curve=self._curve),
                              dh_security_level=self._dh_security_level),
                              credentials=[entry for _, entry in self._credential_history]),
                          self._sni_callback is not None,
                          _host.Tls13Settings(post_handshake_auth=self._post_handshake_auth,
                                              num_tickets=self._num_tickets),
                          self._host_directories(directories))
        try:
            _invoke(context.load_default_trust, _host.DefaultTrustSources(*self._default_sources))
            _invoke(context.update_protocol_settings, self._settings())
            if self._verify_flags & 0x700 and not self._verify_flags & 0x80:
                # A private constructor's set_flags normalizes policy bits.
                # Reconcile the effective mask produced by prior guest deltas
                # before publishing this context or creating its first SSL.
                _invoke(context.update_policy, self._policy())
        except BaseException:
            context.__exit__(None, None, None)
            raise
        previous = self._host_context
        self._host_context, self._cache_key = context, key
        self._store_snapshot_key = (self._ca, self._default_certs, directories, self._default_sources)
        self._policy_snapshot_key = self._policy_key()
        self._settings_snapshot_key = self._settings_key()
        self._tls13_snapshot_key = self._tls13_key()
        if previous is not None:
            previous.__exit__(None, None, None)
        return context

    def session_stats(self):
        stats = _invoke(self._get_host_context(self.protocol == PROTOCOL_TLS_SERVER).session_stats)
        return {name: getattr(stats, name) for name in (
            "number", "connect", "connect_good", "connect_renegotiate", "accept",
            "accept_good", "accept_renegotiate", "hits", "misses", "timeouts", "cache_full")}


class SSLSession(metaclass=_ImmutableType):
    _immutable_type = True
    __slots__ = ("_context", "_handle")

    def __init__(self, *args, **kwargs):
        raise TypeError("SSLSession cannot be instantiated directly")

    def __init_subclass__(cls, **kwargs):
        raise TypeError("SSLSession is not an acceptable base type")

    @classmethod
    def _create(cls, context, handle):
        self = cls.__new__(cls)
        self._context, self._handle = context, handle
        return self

    def _info(self):
        return _invoke(self._handle.info)

    id = property(lambda self: self._info().id)
    time = property(lambda self: self._info().time)
    timeout = property(lambda self: self._info().timeout)
    has_ticket = property(lambda self: self._info().has_ticket)
    ticket_lifetime_hint = property(lambda self: self._info().ticket_lifetime_hint)

    def __eq__(self, other):
        return self.id == other.id if type(other) is SSLSession else NotImplemented

    def __del__(self):
        try:
            handle = getattr(self, "_handle", None)
            if handle is not None:
                handle.__exit__(None, None, None)
                self._handle = None
        except Exception:
            pass


class SSLObject:
    def __init__(self, *args, **kwargs):
        raise TypeError("SSLObject has no public constructor; use SSLContext.wrap_bio()")

    @classmethod
    def _create(cls, incoming, outgoing, server_side=False, server_hostname=None, session=None, context=None):
        self = cls.__new__(cls)
        connection = context._wrap_bio(incoming, outgoing, server_side=server_side,
                                      server_hostname=server_hostname, session=session)
        try:
            SSLObject._initialize(self, context, incoming, outgoing, connection,
                                  bool(server_side), server_hostname)
        except BaseException:
            connection.__exit__(None, None, None)
            object.__setattr__(self, "_connection", None)
            incoming._owner = outgoing._owner = None
            raise
        return self

    def _initialize(self, context, incoming, outgoing, connection, server_side, name):
        self._context, self._incoming, self._outgoing = context, incoming, outgoing
        self._connection = connection
        self._server_side, self._server_hostname = server_side, name
        self._fed_eof = False
        self._sni_origin_context = context
        self._sni_owner = None
        self._in_callback = False
        self._callback_error = None
        # OpenSSL copies callback installation at SSL_new. The user callable
        # itself remains live on the context, including disable/re-enable.
        self._messages_installed = context._msg_callback is not None
        self._in_message_callback = False
        self._message_error = None
        self._defer_message_errors = False
        self._message_driver_active = False
        self._message_raised_error = None
        # Mark ownership without creating a cycle that would retain the host
        # resource until guest garbage collection.
        incoming._owner = outgoing._owner = True

    @property
    def context(self):
        return self._context

    @context.setter
    def context(self, context):
        if not isinstance(context, SSLContext):
            raise TypeError("The value must be a SSLContext")
        if self._connection is None:
            raise ValueError("TLS object is closed")
        _invoke(self._connection.set_context, context._get_host_context(self._server_side))
        self._context = context
        self._messages_installed = context._msg_callback is not None

    def _run_message_callbacks(self, tls_error=None):
        try:
            messages = _invoke(self._connection.take_messages)
        except BaseException as error:
            self._message_error = error
            self._finish_message_error(tls_error)
            return
        for message in messages:
            callback = self.context._msg_callback
            if callback is None:
                continue
            data = bytes(message.data)
            content = _enum_or_int(_TLSContentType, message.content_type)
            version = message.version
            kind = -1
            if content == _TLSContentType.HEADER:
                if len(data) >= 3:
                    version = int.from_bytes(data[1:3], 'big')
                    kind = data[0]
                kind = _enum_or_int(_TLSContentType, kind)
            elif content == _TLSContentType.ALERT:
                kind = _enum_or_int(_TLSAlertType, data[1] if len(data) >= 2 else -1)
            else:
                if content == _TLSContentType.CHANGE_CIPHER_SPEC:
                    kind = _TLSMessageType.CHANGE_CIPHER_SPEC
                elif content in (_TLSContentType.HANDSHAKE, _TLSContentType.INNER_CONTENT_TYPE) and data:
                    kind = _enum_or_int(_TLSMessageType, data[0])
            owner = self if self._sni_owner is None else self._sni_owner()
            self._in_message_callback = True
            try:
                version = _enum_or_int(TLSVersion, version)
                if getattr(self.context, '_raw_message_enums', False):
                    version, content, kind = int(version), int(content), int(kind)
                callback(owner, 'write' if message.write else 'read',
                         version, content, kind, data)
            except BaseException as error:
                # CPython continues delivery and retains the last exception.
                self._message_error = error
            finally:
                self._in_message_callback = False
        self._finish_message_error(tls_error)

    def _finish_message_error(self, tls_error=None):
        callback_error = self._message_error
        if callback_error is not None:
            if self._defer_message_errors and isinstance(tls_error, (SSLWantReadError, SSLWantWriteError)):
                return
            self._message_error = None
            if tls_error is not None:
                # Native exception bookkeeping bypasses subclass hooks. Keep
                # the socket retry marker on our engine, never on user errors.
                BaseException.__context__.__set__(callback_error, BaseException.__context__.__get__(tls_error))
                BaseException.__context__.__set__(tls_error, callback_error)
                if self._message_driver_active:
                    self._message_raised_error = tls_error
                raise tls_error
            if self._message_driver_active:
                self._message_raised_error = callback_error
            raise callback_error

    def _run_sni_callback(self, request):
        # OpenSSL uses the current context's callback, falling back to the
        # initial context when the replacement has no callback installed.
        origin = self.context if self.context.sni_callback is not None else self._sni_origin_context
        callback = origin.sni_callback
        owner = self if self._sni_owner is None else self._sni_owner()
        if owner is None:
            # CPython rejects a collected weak owner without invoking user
            # SNI code. Let OpenSSL emit the fatal alert on its next step.
            _invoke(self._connection.resolve_sni, int(ALERT_DESCRIPTION_INTERNAL_ERROR))
            return
        self._in_callback = True
        alert = None
        try:
            result = callback(owner, request.server_name, origin) if callback is not None else None
            if result is not None:
                value = operator.index(result)
                if not -(1 << 63) <= value < (1 << 63):
                    raise OverflowError("callback alert exceeds signed long")
                alert = value & 0xff
        except BaseException as error:
            self._callback_error = error
            alert = int(ALERT_DESCRIPTION_INTERNAL_ERROR)
        finally:
            self._in_callback = False
        _invoke(self._connection.resolve_sni, alert)

    @property
    def server_side(self):
        return self._server_side

    @property
    def server_hostname(self):
        return self._server_hostname

    @property
    def session(self):
        if self._connection is None:
            raise ValueError("TLS object is closed")
        handle = _invoke(self._connection.get_session)
        return None if handle is None else SSLSession._create(self.context, handle)

    @session.setter
    def session(self, value):
        if type(value) is not SSLSession:
            raise TypeError("session must be an SSLSession")
        if value._context is not self.context:
            raise ValueError("Session refers to a different SSLContext")
        if self._connection is None:
            raise ValueError("TLS object is closed")
        _invoke(self._connection.set_session, value._handle)

    @property
    def session_reused(self):
        if self._connection is None:
            raise ValueError("TLS object is closed")
        return _invoke(self._connection.session_reused)

    def _operation(self, method, *args):
        if self._connection is None:
            raise ValueError("TLS object is closed")
        if self._in_callback:
            raise SSLError(SSL_ERROR_SSL, "TLS I/O is unavailable inside SNI callback")
        if self._in_message_callback:
            raise SSLError(SSL_ERROR_SSL, "TLS I/O is unavailable inside message callback")
        # Verification uses the live context store. Refresh permitted guest
        # directory bytes before handshake/read/write, including TLS 1.3 PHA.
        self.context._sync_trust()
        while True:
            available = _LIMIT - self._info().input_pending
            if self._incoming.pending and available:
                # Preserve the queue when the host refuses the input.
                data = bytes(self._incoming._buffer[:available])
                _invoke(self._connection.feed, data)
                self._incoming.read(len(data))
            if self._incoming.eof and not self._fed_eof:
                _invoke(self._connection.feed_eof)
                self._fed_eof = True
            log_file = self.context._keylog_file
            _invoke(self._connection.keylog_enabled, log_file is not None)
            _invoke(self._connection.messages_enabled,
                    self._messages_installed and self.context._msg_callback is not None)
            tls_error = None
            try:
                return _invoke(getattr(self._connection, method), *args)
            except SSLWantReadError as error:
                tls_error = error
                if self._server_side:
                    request = _invoke(self._connection.pending_sni)
                    if request is not None:
                        # ClientHello diagnostics precede a guest SNI callback
                        # that may replace the context used by later messages.
                        self._run_message_callbacks(error)
                        self._run_sni_callback(request)
                        continue
                # There may be more buffered guest input than fits in one
                # host transfer. Consume it before reporting network readiness.
                if not self._incoming.pending or self._info().input_pending == _LIMIT:
                    raise
            except SSLError as error:
                tls_error = error
                if isinstance(error, SSLCertVerificationError) and error.verify_code in (62, 64):
                    kind = 'Hostname' if error.verify_code == 62 else 'IP address'
                    error.verify_message = "%s mismatch, certificate is not valid for '%s'." % (kind, self.server_hostname)
                if self._callback_error is not None:
                    cause, self._callback_error = self._callback_error, None
                    raise error from cause
                raise
            finally:
                try:
                    while True:
                        data = _invoke(self._connection.drain, _LIMIT)
                        if not data:
                            break
                        self._outgoing.write(data)
                finally:
                    try:
                        lines = bytes(_invoke(self._connection.take_keylog))
                        if lines and log_file is not None:
                            log_file.write(lines)
                            log_file.flush()
                    except Exception as log_error:
                        if tls_error is not None:
                            # CPython keeps the TLS/readiness exception primary
                            # when its keylog callback also failed. Keep any SNI
                            # callback cause in the chain, without a context cycle.
                            log_error.__context__ = tls_error.__cause__
                            raise tls_error from log_error
                        raise
                    finally:
                        self._run_message_callbacks(tls_error)

    def do_handshake(self):
        return self._operation("handshake")

    def compression(self):
        if self._connection is None:
            raise ValueError("TLS object is closed")
        return _invoke(self._connection.compression)

    def selected_npn_protocol(self):
        warnings.warn("ssl NPN is deprecated, use ALPN instead", DeprecationWarning, stacklevel=2)
        return None

    def verify_client_post_handshake(self):
        if self._connection is None:
            raise ValueError("TLS object is closed")
        return _invoke(self._connection.verify_client_post_handshake)

    def read(self, len=1024, buffer=None):
        size, view = _prepare_tls_read(len, buffer)
        if size == 0:
            if self._connection is None:
                raise ValueError("TLS object is closed")
            if self._in_callback or self._in_message_callback:
                raise SSLError(SSL_ERROR_SSL, "TLS I/O is unavailable inside callback")
            # CPython does not feed BIO bytes or enter OpenSSL for an empty
            # destination; queued protocol events and ciphertext stay intact.
            return b"" if buffer is None else 0
        try:
            data = self._operation("read", min(size, _LIMIT))
        except SSLZeroReturnError:
            data = b""
        if buffer is None:
            return data
        view[:len_bytes(data)] = data
        return len_bytes(data)

    def write(self, data):
        # Validate the buffer protocol before TLS I/O. Copy only the bounded
        # transfer, rather than first materializing an arbitrarily large input.
        return self._operation("write", bytes(_readable_buffer(data)[:_LIMIT]))

    def unwrap(self):
        return self._operation("shutdown")

    def _info(self):
        if self._connection is None:
            raise ValueError("TLS object is closed")
        return _invoke(self._connection.info)

    def version(self):
        return self._info().version

    def cipher(self):
        if self._connection is None:
            raise ValueError("TLS object is closed")
        info = _invoke(self._connection.cipher_info)
        return None if info is None else (info.name, info.protocol, info.strength_bits)

    def selected_alpn_protocol(self):
        if self._connection is None:
            raise ValueError("TLS object is closed")
        data = _invoke(self._connection.selected_alpn_bytes)
        return None if data is None else bytes(data).decode('utf-8')

    def get_channel_binding(self, cb_type="tls-unique"):
        _check_channel_binding_type(cb_type)
        if self._connection is None:
            raise ValueError("TLS object is closed")
        return _invoke(self._connection.channel_binding)

    def shared_ciphers(self):
        if self._connection is None:
            raise ValueError("TLS object is closed")
        ciphers = _invoke(self._connection.shared_ciphers)
        return None if ciphers is None else [(cipher.name, cipher.protocol, cipher.strength_bits) for cipher in ciphers]

    def pending(self):
        return self._info().pending

    @property
    def _sslobj(self):
        # Certificate-chain users of CPython's internal TLS object can use
        # the same metadata accessors on this guest engine.
        return self

    def get_verified_chain(self):
        if self._connection is None:
            raise ValueError("TLS object is closed")
        from tarantool__ssl import _chain
        return _chain(self._connection, True)

    def get_unverified_chain(self):
        if self._connection is None:
            raise ValueError("TLS object is closed")
        from tarantool__ssl import _chain
        return _chain(self._connection, False)

    def getpeercert(self, binary_form=False):
        if binary_form:
            info = self._info()
            if info.version is None:
                raise ValueError("handshake not done yet")
            return info.peer_cert_der
        if self._connection is None:
            raise ValueError("TLS object is closed")
        certificate = _invoke(self._connection.peer_certificate)
        if certificate is None:
            return None
        if isinstance(certificate, _host.PeerCertificate_Unverified):
            return {}
        return _certificate_dict(certificate.value)

    def close(self):
        if getattr(self, '_in_message_callback', False):
            raise SSLError(SSL_ERROR_SSL, "TLS close is unavailable inside message callback")
        if getattr(self, '_in_callback', False):
            raise SSLError(SSL_ERROR_SSL, "TLS close is unavailable inside SNI callback")
        if self._connection is not None:
            self._connection.__exit__(None, None, None)
            self._connection = None
            self._incoming._owner = self._outgoing._owner = None

    def __del__(self):
        try:
            self.close()
        except Exception:
            pass


len_bytes = len


def _readable_buffer(data):
    view = memoryview(data)
    if not view.c_contiguous:
        raise BufferError("underlying buffer is not C-contiguous")
    return view.cast("B")


def _prepare_tls_read(size, buffer):
    size = operator.index(size)
    if not -sys.maxsize - 1 <= size <= sys.maxsize:
        raise OverflowError("Python int too large to convert to C ssize_t")
    view = None
    if buffer is not None:
        view = memoryview(buffer)
        if view.readonly or not view.c_contiguous:
            raise TypeError("buffer must be writable and C-contiguous")
        view = view.cast("B")
        if size <= 0 or size > view.nbytes:
            size = view.nbytes
    elif size < 0:
        raise ValueError("size must be non-negative")
    return size, view


def _check_channel_binding_type(cb_type):
    if not isinstance(cb_type, str):
        raise TypeError("channel binding type must be str")
    if cb_type != "tls-unique":
        raise ValueError("%r channel binding type not implemented" % cb_type)


def _password_bytes(password, callback=False):
    if isinstance(password, str):
        value = password.encode("utf-8")
    elif isinstance(password, (bytes, bytearray)):
        value = bytes(password)
    else:
        raise TypeError("password callback must return a string" if callback else
                        "password should be a string or bytes, or a callable returning one")
    if len(value) > 1023:
        raise ValueError("password cannot be longer than 1023 bytes")
    return value


def _unsigned_mask(value):
    if not isinstance(value, int):
        raise TypeError("options must be an integer")
    value = operator.index(value)
    if not 0 <= value <= 0xffffffffffffffff:
        raise OverflowError("unsigned 64-bit mask out of range")
    return value


def _certificate_dict(details):
    result = {
        "subject": _certificate_name(details.subject),
        "issuer": _certificate_name(details.issuer),
        "version": details.version,
        "serialNumber": details.serial_number,
        "notBefore": details.not_before,
        "notAfter": details.not_after,
    }
    if details.subject_alt_name is not None:
        result["subjectAltName"] = tuple(
            (name.kind, _certificate_name(name.value.value)
             if isinstance(name.value, _host.AlternativeNameValue_Name)
             else name.value.value)
            for name in details.subject_alt_name
        )
    for key, value in (("OCSP", details.ocsp), ("caIssuers", details.ca_issuers),
                       ("crlDistributionPoints", details.crl_distribution_points)):
        if value is not None:
            result[key] = tuple(value)
    return result


def _certificate_name(name):
    return tuple(tuple((attribute.name, attribute.value.value) for attribute in rdn)
                 for rdn in name)


class SSLSocket(_socket_module.socket):
    """TLS on the CPython socket, which componentize-py routes through WASI.

    makefile() and its reference counting come from socket.socket. Encrypted
    bytes remain queued until accepted by the underlying socket.
    """
    def __init__(self, *args, **kwargs):
        raise TypeError("SSLSocket has no public constructor; use SSLContext.wrap_socket()")

    @classmethod
    def _create(cls, sock, server_side=False, do_handshake_on_connect=True,
                suppress_ragged_eofs=True, server_hostname=None, context=None, session=None):
        if sock.fileno() < 0:
            raise OSError(errno.EBADF, "Bad file descriptor")
        if sock.type != _socket_module.SOCK_STREAM:
            raise NotImplementedError("only stream sockets are supported")
        if getattr(sock, "_io_refs", 0):
            raise ValueError("cannot wrap a socket with open file objects")
        if server_side and session is not None:
            raise ValueError("session is client-only")
        timeout = sock.gettimeout()
        try:
            sock.getpeername()
            connected = True
        except OSError as error:
            if error.errno != errno.ENOTCONN:
                raise
            connected = False
        incoming, outgoing = MemoryBIO(), MemoryBIO()
        self = cls.__new__(cls, family=sock.family, type=sock.type,
                           proto=sock.proto, fileno=sock.fileno())
        # sslobject_class affects wrap_bio only, just as in CPython. Socket
        # peers always use the adapter's internal BIO engine.
        obj = SSLObject._create(incoming, outgoing, server_side=server_side,
                               server_hostname=context._encode_hostname(server_hostname),
                               session=session if connected else None, context=context)
        fd = None
        try:
            self._sslobj = obj if connected else None
            obj._sni_owner = weakref.ref(self)
            self._incoming, self._outgoing = incoming, outgoing
            self._context = context
            self.server_side, self.server_hostname = bool(server_side), obj.server_hostname
            self.do_handshake_on_connect = bool(do_handshake_on_connect)
            self.suppress_ragged_eofs = bool(suppress_ragged_eofs)
            self._connected = False
            self._session = session
            self._session_pending = not connected
            if not connected:
                obj.close()
            self._record_header = bytearray()
            self._record_remaining = 5
            self._reading_header = True
            # Transfer ownership exactly once; the original socket becomes closed.
            fd = sock.detach()
            # socket.socket.__init__ also assigns overridable Python attributes.
            # Mark native ownership before those assignments can close/reuse fd.
            _socket.socket.__init__(self, sock.family, sock.type, sock.proto, fileno=fd)
            fd = None  # The native socket now owns it, including later close().
            self._io_refs = 0
            self._closed = False
            self.settimeout(timeout)
            try:
                self.getpeername()
            except OSError as error:
                if error.errno != errno.ENOTCONN:
                    raise
            else:
                self._connected = True
            if self._connected and self.do_handshake_on_connect:
                if timeout == 0:
                    raise ValueError("do_handshake_on_connect requires a blocking socket")
                self.do_handshake()
            return self
        except BaseException:
            try:
                obj.close()
            finally:
                if _socket_module.socket.fileno(self) >= 0:
                    _socket_module.socket._real_close(self)
                elif fd is not None:
                    _socket_module.close(fd)
            raise

    @property
    def context(self):
        return self._context

    @context.setter
    def context(self, context):
        if not isinstance(context, SSLContext):
            raise TypeError("The value must be a SSLContext")
        if self._sslobj is not None:
            self._sslobj.context = context
        self._context = context

    @property
    def session(self):
        return None if self._sslobj is None or not self._connected else self._sslobj.session

    @session.setter
    def session(self, value):
        self._session = value
        if self._sslobj is not None and not self._session_pending:
            self._sslobj.session = value
        elif self._sslobj is not None:
            self._session_pending = True

    def _apply_pending_session(self):
        if self._session_pending:
            if self._sslobj is None:
                saved = {name: object.__getattribute__(self, name) for name in (
                    "_sslobj", "_incoming", "_outgoing", "_record_header",
                    "_record_remaining", "_reading_header", "_session_pending")}
                incoming, outgoing = MemoryBIO(), MemoryBIO()
                obj = SSLObject._create(incoming, outgoing,
                    server_side=self.server_side, server_hostname=self.server_hostname,
                    session=self._session, context=self._context)
                try:
                    obj._sni_owner = weakref.ref(self)
                    self._sslobj = obj
                    self._incoming, self._outgoing = incoming, outgoing
                    self._record_header = bytearray()
                    self._record_remaining = 5
                    self._reading_header = True
                    self._session_pending = False
                except BaseException:
                    try:
                        obj.close()
                    finally:
                        # Custom assignment hooks must not prevent rollback or
                        # keep an unowned host resource alive in a traceback.
                        for name, value in saved.items():
                            object.__setattr__(self, name, value)
                    raise
                return
            elif self._session is not None:
                self._sslobj.session = self._session
            self._session_pending = False

    def _discard_failed_connect(self):
        self._sslobj.close()
        self._sslobj = None
        self._session_pending = True

    @property
    def session_reused(self):
        return None if self._sslobj is None or not self._connected else self._sslobj.session_reused

    def _check_tls(self):
        if self.fileno() < 0:
            raise OSError(errno.EBADF, "Bad file descriptor")
        if not self._connected:
            # Handles a nonblocking connect completed by the caller's poll.
            self.getpeername()
            self._connected = True
        self._apply_pending_session()
        if self._sslobj is None:
            raise ValueError("TLS socket is closed or unwrapped")

    def _raw_socket(self):
        return self

    def _flush_output(self, deadline=None):
        while self._outgoing.pending:
            if deadline is not None and time.monotonic() >= deadline:
                raise TimeoutError("TLS operation timed out")
            try:
                count = _socket_module.socket.send(self._raw_socket(), bytes(self._outgoing._buffer[:65536]))
            except BlockingIOError:
                raise SSLWantWriteError(SSL_ERROR_WANT_WRITE, "The operation did not complete (write)") from None
            if count == 0:
                raise ConnectionError("socket closed during TLS write")
            self._outgoing.read(count)

    def _receive_record_bytes(self):
        # Feed only one TLS record at a time. A close_notify and plaintext
        # following unwrap can share one TCP packet; plaintext must stay in
        # the underlying socket, including when callers detach its descriptor.
        data = _socket_module.socket.recv(self._raw_socket(), self._record_remaining)
        if not data:
            self._incoming.write_eof()
            return
        self._record_remaining -= len(data)
        if self._reading_header:
            self._record_header.extend(data)
            if self._record_remaining == 0:
                self._record_remaining = int.from_bytes(self._record_header[3:5], "big")
                self._record_header.clear()
                self._reading_header = False
        if self._record_remaining == 0:
            self._record_remaining = 5
            self._reading_header = True
        self._incoming.write(data)

    def _wait_io(self, timeout, deadline, *, reading=False, writing=False):
        if timeout == 0:
            if writing:
                raise SSLWantWriteError(SSL_ERROR_WANT_WRITE, "The operation did not complete (write)")
            if reading:
                raise SSLWantReadError(SSL_ERROR_WANT_READ, "The operation did not complete (read)")
            raise SSLWantWriteError(SSL_ERROR_WANT_WRITE, "The operation did not complete (write)")
        left = None if deadline is None else deadline - time.monotonic()
        if left is not None and left <= 0:
            raise TimeoutError("TLS operation timed out")
        ready = select.select([self] if reading else [], [self] if writing else [], [], left)
        if not ready[0] and not ready[1]:
            raise TimeoutError("TLS operation timed out")

    def _run(self, method, *args, accepted_write=False):
        self._check_message_callback()
        self._check_tls()
        # Keep a borrowed private socket alive only for this operation.
        raw = self._raw_socket()
        try:
            timeout = self.gettimeout()
            deadline = None if timeout is None or timeout == 0 else time.monotonic() + timeout

            def flush(opportunistic=False):
                while True:
                    try:
                        self._flush_output(deadline)
                        return
                    except SSLWantWriteError:
                        if opportunistic:
                            return
                        self._wait_io(timeout, deadline, writing=True)

            # Readiness waiting happens in select with a single operation deadline.
            # Each raw send/recv is nonblocking, so reads can drain input while
            # ciphertext is queued, even after switching back to blocking mode.
            _socket_module.socket.settimeout(raw, 0)
            engine = self._sslobj
            engine._defer_message_errors = timeout != 0
            engine._message_driver_active = True
            try:
                while True:
                    if deadline is not None and time.monotonic() >= deadline:
                        raise TimeoutError("TLS operation timed out")
                    flush(opportunistic=method == "read")
                    try:
                        result = getattr(self._sslobj, method)(*args)
                    except SSLWantWriteError as error:
                        if engine._message_raised_error is error:
                            flush(opportunistic=True)
                            raise
                        flush()
                        continue
                    except SSLWantReadError as error:
                        if engine._message_raised_error is error:
                            flush(opportunistic=True)
                            raise
                        flush(opportunistic=method == "read")
                        try:
                            self._receive_record_bytes()
                        except BlockingIOError:
                            self._wait_io(timeout, deadline, reading=True,
                                          writing=bool(self._outgoing.pending))
                        continue
                    except BaseException:
                        # OpenSSL can queue a fatal alert before reporting failure.
                        # Send what is immediately writable without replacing the
                        # original TLS failure with a transport/readiness error.
                        try:
                            flush(opportunistic=True)
                        except OSError:
                            pass
                        raise
                    if method == "read":
                        # Plaintext has already been consumed (and possibly copied
                        # into the caller's buffer). Preserve that successful result
                        # even if crypto/host scheduling crossed the deadline. Any
                        # new ciphertext is flushed by the next socket operation.
                        return result
                    if deadline is not None and time.monotonic() >= deadline:
                        raise TimeoutError("TLS operation timed out")
                    # A successful nonblocking write has consumed plaintext;
                    # retain queued ciphertext while reporting that accepted count.
                    flush(opportunistic=method == "read" or (accepted_write and timeout == 0))
                    return result
            finally:
                try:
                    # A private callback may close the caller-owned raw socket.
                    # Restore only a live descriptor, preserving the TLS/IO error.
                    if _socket_module.socket.fileno(raw) >= 0:
                        _socket_module.socket.settimeout(raw, timeout)
                finally:
                    engine._defer_message_errors = False
                    engine._message_driver_active = False
                    engine._message_raised_error = None
                    engine._finish_message_error(sys.exception())
        finally:
            # Exception tracebacks must not retain borrowed raw sockets.
            raw = None

    def do_handshake(self, block=False):
        self._check_message_callback()
        previous = self.gettimeout()
        if block and previous == 0:
            self.settimeout(None)
        try:
            return self._run("do_handshake")
        finally:
            self.settimeout(previous)

    def read(self, len=1024, buffer=None):
        if self._sslobj is None or self._session_pending:
            raise ValueError("Read on closed or unwrapped SSL socket.")
        self._check_message_callback()
        size, view = _prepare_tls_read(len, buffer)
        if size == 0:
            if self._sslobj._in_callback:
                raise SSLError(SSL_ERROR_SSL, "TLS I/O is unavailable inside SNI callback")
            # Native zero reads neither wait for readiness nor send pending
            # ciphertext. Keep the validated destination alive until return.
            return b"" if buffer is None else 0
        try:
            return self._run("read", size, view)
        except SSLEOFError:
            if not self.suppress_ragged_eofs:
                raise
            return b"" if buffer is None else 0

    def write(self, data):
        if self._sslobj is None or self._session_pending:
            raise ValueError("Write on closed or unwrapped SSL socket.")
        self._check_message_callback()
        view = _readable_buffer(data)[:_LIMIT]
        return self._run("write", view, accepted_write=True)

    def recv(self, buflen=1024, flags=0):
        if self._sslobj is None or self._session_pending:
            return _socket_module.socket.recv(self, buflen, flags)
        if flags:
            raise ValueError("non-zero flags are not allowed on SSLSocket")
        return self.read(buflen)

    def recv_into(self, buffer, nbytes=None, flags=0):
        if nbytes is None:
            nbytes = (memoryview(buffer).nbytes or 1024) if buffer is not None else 1024
        if self._sslobj is None or self._session_pending:
            return _socket_module.socket.recv_into(self, buffer, nbytes, flags)
        if flags:
            raise ValueError("non-zero flags are not allowed on SSLSocket")
        return self.read(nbytes, buffer)

    def send(self, data, flags=0):
        if self._sslobj is None or self._session_pending:
            return _socket_module.socket.send(self, data, flags)
        if flags:
            raise ValueError("non-zero flags are not allowed on SSLSocket")
        return self.write(data)

    def sendall(self, data, flags=0):
        self._check_message_callback()
        if self._sslobj is None or self._session_pending:
            return _socket_module.socket.sendall(self, data, flags)
        if flags:
            raise ValueError("non-zero flags are not allowed on SSLSocket")
        view = memoryview(data).cast("B")
        timeout = self.gettimeout()
        deadline = None if timeout is None or timeout == 0 else time.monotonic() + timeout
        try:
            while view:
                if deadline is not None:
                    left = deadline - time.monotonic()
                    if left <= 0:
                        raise TimeoutError("TLS sendall timed out")
                    self.settimeout(left)
                count = self.send(view)
                if count == 0:
                    raise ConnectionError("socket closed during TLS write")
                view = view[count:]
            # Even the final successful write may have queued ciphertext.
            _socket_module.socket.settimeout(self, 0)
            while True:
                try:
                    self._flush_output(deadline)
                    break
                except SSLWantWriteError:
                    self._wait_io(timeout, deadline, writing=True)
        finally:
            self.settimeout(timeout)

    def connect(self, address):
        if self.server_side:
            raise ValueError("cannot connect in server mode")
        if self._connected or not self._session_pending:
            raise ValueError("attempt to connect already-connected SSLSocket")
        self._apply_pending_session()
        try:
            _socket_module.socket.connect(self, address)
            self._connected = True
            if self.do_handshake_on_connect:
                self.do_handshake()
        except (OSError, ValueError):
            self._discard_failed_connect()
            raise

    def sendfile(self, file, offset=0, count=None):
        self._check_message_callback()
        if self._sslobj is not None:
            # The stdlib fallback sends bounded chunks through self.send(),
            # preserving TLS and native offset/count/file-position semantics.
            return self._sendfile_use_send(file, offset, count)
        return _socket_module.socket.sendfile(self, file, offset, count)

    def connect_ex(self, address):
        if self.server_side:
            raise ValueError("cannot connect in server mode")
        if self._connected or not self._session_pending:
            raise ValueError("attempt to connect already-connected SSLSocket")
        self._apply_pending_session()
        try:
            result = _socket_module.socket.connect_ex(self, address)
            if result == 0:
                self._connected = True
                if self.do_handshake_on_connect:
                    self.do_handshake()
            return result
        except (OSError, ValueError):
            self._discard_failed_connect()
            raise

    def accept(self):
        raw, address = _socket_module.socket.accept(self)
        try:
            peer = self.context.wrap_socket(raw, server_side=True,
                                            do_handshake_on_connect=self.do_handshake_on_connect,
                                            suppress_ragged_eofs=self.suppress_ragged_eofs)
            return peer, address
        except BaseException:
            raw.close()
            raise

    def unwrap(self):
        self._run("unwrap")
        self._sslobj.close()
        self._sslobj = None
        return self

    def shutdown(self, how):
        self._check_message_callback()
        if self._sslobj is not None:
            self._sslobj.close()
            self._sslobj = None
        return _socket_module.socket.shutdown(self, how)

    def _check_message_callback(self):
        engine = getattr(self, '_sslobj', None)
        if engine is not None and engine._in_message_callback:
            raise SSLError(SSL_ERROR_SSL, "TLS socket I/O and close are unavailable inside message callback")
        if engine is not None and engine._in_callback:
            raise SSLError(SSL_ERROR_SSL, "TLS socket I/O and close are unavailable inside SNI callback")

    def close(self):
        self._check_message_callback()
        return _socket_module.socket.close(self)

    def _real_close(self):
        self._check_message_callback()
        try:
            if getattr(self, "_sslobj", None) is not None:
                self._sslobj.close()
                self._sslobj = None
        finally:
            _socket_module.socket._real_close(self)

    def detach(self):
        if self._sslobj is not None:
            raise NotImplementedError("unwrap TLS before detaching the socket")
        return _socket_module.socket.detach(self)

    def pending(self):
        return 0 if self._sslobj is None else self._sslobj.pending()

    def version(self):
        return None if self._sslobj is None else self._sslobj.version()

    def cipher(self):
        return None if self._sslobj is None else self._sslobj.cipher()

    def selected_alpn_protocol(self):
        return None if self._sslobj is None else self._sslobj.selected_alpn_protocol()

    def compression(self):
        return None if self._sslobj is None else self._sslobj.compression()

    def selected_npn_protocol(self):
        warnings.warn("ssl NPN is deprecated, use ALPN instead", DeprecationWarning, stacklevel=2)
        return None

    def get_channel_binding(self, cb_type="tls-unique"):
        if self._sslobj is None or not self._connected:
            if cb_type != "tls-unique":
                raise ValueError("%r channel binding type not implemented" % cb_type)
            return None
        return self._sslobj.get_channel_binding(cb_type)

    def shared_ciphers(self):
        return None if self._sslobj is None else self._sslobj.shared_ciphers()

    def verify_client_post_handshake(self):
        # Before connect(), CPython has no SSL wrapper yet. Our deferred BIO
        # placeholder is likewise unavailable until the first connect attempt.
        # PHA only queues a request, so TCP readiness must not be checked here.
        if self._sslobj is None or self._session_pending:
            raise ValueError("No SSL wrapper around socket")
        return self._sslobj.verify_client_post_handshake()

    def getpeercert(self, binary_form=False):
        self._check_tls()
        return self._sslobj.getpeercert(binary_form)

    # Inherited datagram/vector APIs would bypass TLS and expose raw TCP.
    def _unsupported_io(self, *args, **kwargs):
        raise NotImplementedError("operation is unavailable on SSLSocket")

    def _unsupported_datagram(self, *args, **kwargs):
        self.getpeername()  # Preserve ENOTCONN before refusing raw TCP access.
        return self._unsupported_io(*args, **kwargs)

    sendto = recvfrom = recvfrom_into = _unsupported_datagram
    sendmsg = recvmsg = recvmsg_into = _unsupported_io
    dup = _unsupported_io


SSLContext.sslobject_class = SSLObject
SSLContext.sslsocket_class = SSLSocket


def _purpose_protocol(purpose):
    if not isinstance(purpose, _ASN1Object):
        raise TypeError(purpose)
    if purpose == Purpose.SERVER_AUTH:
        return PROTOCOL_TLS_CLIENT
    if purpose == Purpose.CLIENT_AUTH:
        return PROTOCOL_TLS_SERVER
    raise ValueError(purpose)


def _environment_keylog(context):
    filename = os.environ.get('SSLKEYLOGFILE')
    if filename and not sys.flags.ignore_environment:
        context.keylog_filename = filename


def create_default_context(purpose=Purpose.SERVER_AUTH, *, cafile=None, capath=None, cadata=None):
    context = SSLContext(_purpose_protocol(purpose))
    if cafile or capath or cadata:
        context.load_verify_locations(cafile, capath, cadata)
    elif context.verify_mode != CERT_NONE:
        context.load_default_certs(purpose)
    _environment_keylog(context)
    return context


_create_default_https_context = create_default_context


def _create_unverified_context(protocol=None, *, cert_reqs=CERT_NONE,
                               check_hostname=False, purpose=Purpose.SERVER_AUTH,
                               certfile=None, keyfile=None, cafile=None, capath=None,
                               cadata=None):
    default_protocol = _purpose_protocol(purpose)
    context = SSLContext(default_protocol if protocol is None else protocol)
    context.check_hostname = check_hostname
    if cert_reqs is not None:
        context.verify_mode = cert_reqs
    if keyfile and not certfile:
        raise ValueError("certfile must be specified")
    if certfile or keyfile:
        context.load_cert_chain(certfile, keyfile)
    if cafile or capath or cadata:
        context.load_verify_locations(cafile, capath, cadata)
    elif context.verify_mode != CERT_NONE:
        context.load_default_certs(purpose)
    _environment_keylog(context)
    return context


_create_stdlib_context = _create_unverified_context


def DER_cert_to_PEM_cert(der_cert_bytes):
    encoded = base64.standard_b64encode(der_cert_bytes).decode("ascii")
    lines = [encoded[offset:offset + 64] for offset in range(0, len(encoded), 64)]
    return "-----BEGIN CERTIFICATE-----\n" + "".join(line + "\n" for line in lines) + "-----END CERTIFICATE-----\n"


def PEM_cert_to_DER_cert(pem_cert_string):
    header, footer = "-----BEGIN CERTIFICATE-----", "-----END CERTIFICATE-----"
    if not pem_cert_string.startswith(header):
        raise ValueError("Invalid PEM encoding; must start with %s" % header)
    if not pem_cert_string.strip().endswith(footer):
        raise ValueError("Invalid PEM encoding; must end with %s" % footer)
    body = pem_cert_string.strip()[len(header):-len(footer)].strip()
    return base64.decodebytes(body.encode("ascii", "strict"))


def cert_time_to_seconds(cert_time):
    months = ("Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec")
    try:
        month = months.index(cert_time[:3].title()) + 1
    except ValueError:
        raise ValueError("time data %r does not match format '%%b %%d %%H:%%M:%%S %%Y GMT'" % cert_time) from None
    parsed = time.strptime(cert_time[3:], " %d %H:%M:%S %Y GMT")
    return calendar.timegm((parsed.tm_year, month) + parsed[2:])


def get_server_certificate(addr, ssl_version=PROTOCOL_TLS_CLIENT, ca_certs=None,
                           timeout=_socket_module._GLOBAL_DEFAULT_TIMEOUT):
    host, port = addr
    context = _create_stdlib_context(ssl_version,
                                   cert_reqs=CERT_REQUIRED if ca_certs else CERT_NONE,
                                   cafile=ca_certs)
    with _socket_module.create_connection((host, port), timeout=timeout) as raw:
        with context.wrap_socket(raw, server_hostname=host) as peer:
            return DER_cert_to_PEM_cert(peer.getpeercert(True))


def install():
    """Provide import ssl before importing libraries that depend on it."""
    sys.modules["ssl"] = sys.modules[__name__]
    import tarantool__ssl
    sys.modules["_ssl"] = tarantool__ssl
    globals()["_ssl"] = tarantool__ssl


# Lazy backend attributes must also be resolved by `from ssl import *`.
__all__ = sorted({name for name in globals() if not name.startswith('_')} |
                 set(_PROTOCOL_FLAGS) | {'OPENSSL_VERSION', 'OPENSSL_VERSION_NUMBER', 'OPENSSL_VERSION_INFO'})
