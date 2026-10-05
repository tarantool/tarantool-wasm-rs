"""Local TLS peer for the Wasm socket tests; needs only host Python stdlib."""
import json
import gzip
import os
import signal
import socket
import ssl
import sys
import threading
import time


def serve_peer(raw, context):
    try:
        with context.wrap_socket(raw, server_side=True) as peer:
            peer.settimeout(5)
            first = peer.recv(1)
            if first == b"B":
                binding = peer.get_channel_binding()
                peer.sendall(len(binding).to_bytes(2, "big") + binding)
                peer.unwrap().close()
                return
            if first == b"T":
                time.sleep(2)
                return
            if first == b"R":
                return  # Close TCP without close_notify.
            if first == b"U":
                plain = peer.unwrap()
                try:
                    plain.sendall(b"plain-after-tls")
                finally:
                    plain.close()
                return
            if first == b"E":
                while True:
                    data = peer.recv(16384)
                    if not data:
                        break
                    peer.sendall(data)
                return
            data = first
            while b"\r\n\r\n" not in data:
                chunk = peer.recv(4096)
                if not chunk:
                    return
                data += chunk
            headers, body = data.split(b"\r\n\r\n", 1)
            length = 0
            for line in headers.split(b"\r\n")[1:]:
                if line.lower().startswith(b"content-length:"):
                    length = int(line.split(b":", 1)[1])
            while len(body) < length:
                body += peer.recv(min(65536, length - len(body)))
            response = body if headers.startswith(b"POST ") else b"wasi-https-ok"
            encoding = b""
            if headers.startswith(b"GET /gzip "):
                response = gzip.compress(response)
                encoding = b"Content-Encoding: gzip\r\n"
            for charset, text in (("cp1252", "caf\u00e9 \u20ac"), ("shift_jis", "\u65e5\u672c\u8a9e"),
                                  ("utf-8-sig", "BOM \u043f\u0440\u0438\u0432\u0435\u0442")):
                if headers.startswith(("GET /" + charset + " ").encode()):
                    response = text.encode(charset)
                    encoding = ("Content-Type: text/plain; charset=" + charset + "\r\n").encode()
            peer.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: " + str(len(response)).encode()
                         + b"\r\n" + encoding + b"Connection: close\r\n\r\n" + response)
            try:
                peer.unwrap().close()
            except (OSError, ValueError):
                pass
    except (OSError, ValueError):
        raw.close()  # Rejected certificates/ACLs can close during handshake.


def close_after_client_hello(raw):
    """Fail TLS after TCP connect, without racing the guest's finish-connect."""
    try:
        with raw:
            deadline = time.monotonic() + 5
            def read_exact(size):
                result = bytearray()
                while len(result) < size:
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        raise TimeoutError('incomplete test ClientHello')
                    raw.settimeout(remaining)
                    chunk = raw.recv(size - len(result))
                    if not chunk:
                        raise EOFError('incomplete test ClientHello')
                    result.extend(chunk)
                return result
            header = read_exact(5)
            if header[0] == 22:  # Handshake record. Drain its payload before FIN.
                read_exact(int.from_bytes(header[3:5], 'big'))
    except (OSError, EOFError):
        pass


def accept_peers(listener, context):
    while True:
        raw, _ = listener.accept()
        if context is None:
            threading.Thread(target=close_after_client_hello, args=(raw,), daemon=True).start()
        else:
            threading.Thread(target=serve_peer, args=(raw, context), daemon=True).start()


def coalesced_peer(raw, context):
    """Put TLS close_notify and subsequent plaintext in one TCP write."""
    incoming, outgoing = ssl.MemoryBIO(), ssl.MemoryBIO()
    tls = context.wrap_bio(incoming, outgoing, server_side=True)
    try:
        raw.settimeout(5)
        for operation in (tls.do_handshake, lambda: tls.read(1)):
            while True:
                try:
                    operation()
                    break
                except ssl.SSLWantReadError:
                    wire = outgoing.read()
                    if wire:
                        raw.sendall(wire)
                    wire = raw.recv(65536)
                    if not wire:
                        return
                    incoming.write(wire)
            wire = outgoing.read()
            if wire:
                raw.sendall(wire)
        try:
            tls.unwrap()
        except ssl.SSLWantReadError:
            pass
        raw.sendall(outgoing.read() + b"plain-after-tls")
        raw.recv(65536)
    except OSError:
        pass
    finally:
        raw.close()


def accept_coalesced(listener, context):
    while True:
        raw, _ = listener.accept()
        threading.Thread(target=coalesced_peer, args=(raw, context), daemon=True).start()


def listener_client(cert_dir, port, reject_first=False):
    def connect_peer():
        deadline = time.monotonic() + 5
        while True:
            try:
                return socket.create_connection(("127.0.0.1", port), timeout=3)
            except ConnectionRefusedError:
                if time.monotonic() >= deadline:
                    raise
                time.sleep(0.01)
    context = ssl.create_default_context(cafile=cert_dir + "/server.crt")
    if reject_first == 'pha':
        context.post_handshake_auth = True
        context.load_cert_chain(cert_dir + '/server.crt', cert_dir + '/server.key')
    if reject_first == "sni-alert-first":
        with connect_peer() as raw:
            try:
                context.wrap_socket(raw, server_hostname="localhost")
            except ssl.SSLError as error:
                assert "UNRECOGNIZED_NAME" in error.reason, repr(error)
            else:
                raise AssertionError("SNI alert was not delivered")
    elif reject_first and reject_first != 'pha':
        with connect_peer() as raw:
            raw.sendall(b"plaintext is not TLS")
    with connect_peer() as raw:
        with context.wrap_socket(raw, server_hostname="localhost") as peer:
            if reject_first == 'pha':
                request = b''
                while len(request) < len(b'auth-request'):
                    data = peer.recv(3)
                    assert data
                    request += data
                assert request == b'auth-request'
            peer.sendall(b"listener-client")
            response = b""
            while len(response) < len(b"listener-server"):
                data = peer.recv(3)
                assert data
                response += data
            assert response == b"listener-server"
            peer.unwrap().close()
    return "listener-client-ok"


def client_main():
    try:
        mode = sys.argv[5]
        result = {"result": listener_client(sys.argv[3], int(sys.argv[4]),
                  mode if mode in ('sni-alert-first', 'pha') else mode == "reject-first")}
    except Exception as error:
        result = {"error": repr(error)}
    temporary = sys.argv[2] + ".tmp"
    with open(temporary, "w") as file:
        json.dump(result, file)
    os.replace(temporary, sys.argv[2])


def main():
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(sys.argv[2] + "/server.crt", sys.argv[2] + "/server.key")
    context.set_alpn_protocols(["http/1.1"])
    strict_context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    strict_context.load_cert_chain(sys.argv[2] + "/leaf.crt", sys.argv[2] + "/server.key")
    strict_context.set_alpn_protocols(["http/1.1"])
    listeners = []
    for ctx in (context, None, strict_context):
        listener = socket.socket()
        listener.bind(("127.0.0.1", 0))
        listener.listen()
        listeners.append(listener)
        threading.Thread(target=accept_peers, args=(listener, ctx), daemon=True).start()
    listener = socket.socket()
    listener.bind(("127.0.0.1", 0))
    listener.listen()
    listeners.append(listener)
    threading.Thread(target=accept_coalesced, args=(listener, context), daemon=True).start()
    state = {"pid": os.getpid(), "tls_port": listeners[0].getsockname()[1],
             "plain_port": listeners[1].getsockname()[1],
             "strict_port": listeners[2].getsockname()[1],
             "coalesced_port": listener.getsockname()[1]}
    temporary = sys.argv[1] + ".tmp"
    with open(temporary, "w") as file:
        json.dump(state, file)
    os.replace(temporary, sys.argv[1])
    signal.pause()


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--client":
        client_main()
    else:
        main()
