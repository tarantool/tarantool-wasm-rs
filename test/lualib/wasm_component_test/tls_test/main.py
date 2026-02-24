from wit_world.imports import (
    instance_network,
    monotonic_clock,
    network,
    poll as wasi_poll,
    tcp_create_socket,
    types as tls,
)
import traceback
from wit_world.types import Err

CONNECT_TIMEOUT_NS = 10_000_000_000
TLS_TIMEOUT_NS = 15_000_000_000
_NONE_UNPACK_ERROR = "cannot unpack non-iterable NoneType object"
# Work around componentize-py resource-drop traps by pinning created resources
# for the short lifetime of the test process.
_LEAKED_RESOURCES = []


def _parse_ipv4(host: str):
    parts = host.split(".")
    if len(parts) != 4:
        return None

    octets = []
    for part in parts:
        if not part.isdigit():
            return None
        value = int(part)
        if value < 0 or value > 255:
            return None
        octets.append(value)

    return tuple(octets)


def _deadline_after(timeout_ns: int) -> int:
    return monotonic_clock.now() + timeout_ns


def _remaining(deadline_ns: int) -> int:
    now = monotonic_clock.now()
    if now >= deadline_ns:
        return 0
    return deadline_ns - now


def _wait_until_ready(target, deadline_ns: int) -> bool:
    remaining = _remaining(deadline_ns)
    if remaining == 0:
        return False

    timeout_pollable = monotonic_clock.subscribe_duration(remaining)
    _LEAKED_RESOURCES.append(timeout_pollable)
    ready = wasi_poll.poll([target, timeout_pollable])
    return 0 in ready


def _is_none_unpack_error(err: Exception) -> bool:
    return isinstance(err, TypeError) and _NONE_UNPACK_ERROR in str(err)


class WitWorld:
    def _tls_handshake(self, host: str, port: int, server_name: str) -> str:
        address = _parse_ipv4(host)
        if address is None:
            return "invalid-host"

        try:
            net = instance_network.instance_network()
            sock = tcp_create_socket.create_tcp_socket(network.IpAddressFamily.IPV4)
            _LEAKED_RESOURCES.extend([net, sock])

            remote = network.IpSocketAddress_Ipv4(
                network.Ipv4SocketAddress(port=port, address=address)
            )
            sock.start_connect(net, remote)
            connect_pollable = sock.subscribe()
            _LEAKED_RESOURCES.append(connect_pollable)
            connect_deadline = _deadline_after(CONNECT_TIMEOUT_NS)

            while True:
                if not _wait_until_ready(connect_pollable, connect_deadline):
                    return "tcp-connect-timeout"

                try:
                    connected = sock.finish_connect()
                except TypeError as err:
                    if _is_none_unpack_error(err):
                        continue
                    raise
                except Err as err:
                    if err.value == network.ErrorCode.WOULD_BLOCK:
                        continue
                    return f"tcp-connect-error:{err.value.name}"

                if connected is None:
                    continue

                input_stream, output_stream = connected
                _LEAKED_RESOURCES.extend([input_stream, output_stream])
                break

            handshake = tls.ClientHandshake(server_name, input_stream, output_stream)
            _LEAKED_RESOURCES.append(handshake)
            finish_deadline = _deadline_after(TLS_TIMEOUT_NS)
            while True:
                try:
                    future = tls.ClientHandshake.finish(handshake)
                    _LEAKED_RESOURCES.append(future)
                    break
                except TypeError as err:
                    if _is_none_unpack_error(err):
                        if _remaining(finish_deadline) == 0:
                            return "tls-finish-timeout"
                        continue
                    raise

            tls_pollable = future.subscribe()
            _LEAKED_RESOURCES.append(tls_pollable)
            tls_deadline = _deadline_after(TLS_TIMEOUT_NS)
            while True:
                if not _wait_until_ready(tls_pollable, tls_deadline):
                    return "tls-handshake-timeout"

                try:
                    result = future.get()
                except TypeError as err:
                    if _is_none_unpack_error(err):
                        continue
                    raise

                if result is None:
                    continue

                if isinstance(result, Err):
                    return "tls-future-error"

                inner = result.value
                if isinstance(inner, Err):
                    return f"tls-handshake-error:{inner.value.to_debug_string()}"

                _LEAKED_RESOURCES.extend(inner.value)
                return "tls-handshake-success"
        except Err as err:
            return f"unexpected-wasi-error:{err.value}"
        except Exception as err:
            tb = traceback.extract_tb(err.__traceback__)
            if tb:
                frames = ",".join(
                    f"{frame.name}@{frame.filename}:{frame.lineno}" for frame in tb[-4:]
                )
                return f"unexpected-error:{err};trace:{frames}"
            return f"unexpected-error:{err}"

    def t_tls_handshake(self, host: str, port: int, server_name: str) -> str:
        return self._tls_handshake(host, port, server_name)

    def t_tls_handshake_plain_fails(self, host: str, port: int) -> str:
        return self._tls_handshake(host, port, "localhost")
