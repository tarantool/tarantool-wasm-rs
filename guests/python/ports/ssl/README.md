# Python SSL port

The SDK installs `tarantool_ssl`/`tarantool__ssl` as `ssl`/`_ssl` before the
application imports them. The API targets CPython 3.12; the pinned componentizer
embeds CPython 3.14.0. Use [the SDK installation instructions](../../README.md) to add the
`docs:adder/ssl-bio@0.1.3` import and package the adapter.

## Ownership

`python/` implements the facade; `wit/ssl-bio.wit` is the sole contract source;
`host/` contains OpenSSL and WIT Host implementations. Core only registers the
optional host and supplies its resource-table guard (`python-ssl-compat`).
All native SSL operations are synchronous under that guard, with no guest
callbacks or awaits while it is held.

TLS uses memory BIOs. Public sockets own guest WASI TCP descriptors; private
`_wrap_socket` borrows a weakly referenced raw socket and never closes it.
The caller must retain and close that socket. Connections/sessions retain native
context references after a context resource is dropped. Explicit close/drop is
available; garbage collection is a fallback.

## Supported behavior

| Area | Coverage |
| --- | --- |
| `MemoryBIO`, `SSLObject` | Buffers, handshake, read/write, shutdown, EOF, metadata and peer chains |
| `SSLSocket` | Connected/deferred connect, TLS listener/accept, timeouts, nonblocking readiness, send/recv, sendall, sendfile, makefile and unwrap |
| Context policy | Generic/fixed TLS protocols, version bounds, verification/hostname flags, cipher metadata, options, DH/ECDH, ALPN and SNI selection |
| Credentials/trust | PEM chains, encrypted keys/password callbacks, PEM/DER CAs, CRLs/AUX trust, lazy hashed CA directories and store inspection |
| Sessions/TLS 1.3 | Metadata, import/reuse, ticket count and post-handshake client authentication |
| Diagnostics | Symbolic error library/reason, verification details, message callbacks and guest key logs |
| Private `_ssl` | Certificate helpers, private contexts/BIOs/sockets, binary ALPN, hostname flags, owner and certificate chains |
| Consumers | stdlib HTTPS, asyncio TLS, urllib3 and Requests fixtures |

Imports and ordinary context construction/settings are safe during
componentization. Host capability queries, credential loading and cipher
introspection require an instantiated component. Compiled legacy protocol
availability does not override OpenSSL security policy; default client/server
contexts use TLS 1.2 minimum and security level 2.

## Files and permissions

Certificate/key/CA/DH files are read through guest preopens. Guest paths never
become host paths. Public PEM parsing rejects password prompts on host stdin;
password callbacks execute in the guest only when an encrypted key needs one.
Omitted passwords raise `SSLError`. Decrypted key bytes remain in guest memory.

`load_default_certs()` explicitly adds host-configured OpenSSL defaults.
Guest `SSL_CERT_FILE` and `SSL_CERT_DIR` independently override the corresponding
source through guest files/snapshots. Empty or missing overrides suppress that
host source. Previously loaded anchors remain loaded. The WIT selector accepts
only file/directory booleans and preserves existing constructors/methods.

WASI network permissions (`inherit_network`, `allowed_ips`, `allowed_ports`)
apply to all TLS sockets. Certifi and key logs require a writable guest preopen,
usually `/tmp`. Key logs contain traffic secrets. Changing context policy/trust
preserves the retained store and session identity; already-created SSL objects
keep settings OpenSSL copies at creation.

Inputs, queues and transfers are bounded. Most individual TLS transfers and
credential/trust inputs are limited to 1 MiB; retained guest trust is limited to
2 MiB/8192 objects, directory snapshots to 1 MiB/4096 files, credentials to
1 MiB/16 key types and message queues to 4096 events/1 MiB per operation.
Oversized data fails explicitly.

## Compatibility limits

Client-side generic-context SNI acknowledgement callbacks are not bridged.
Server SNI/message callbacks execute between host calls; nested TLS I/O is
rejected. SNI callback exceptions are chained to `SSLError`, whereas CPython
uses its unraisable hook. Malformed UTF-8 certificate metadata raises `SSLError`
instead of `UnicodeDecodeError`.

Credential parsing uses memory APIs, so some malformed/encrypted-key errors
and metadata differ from CPython's file loader. Failed loads preserve the
previous complete credentials; native CPython can partially replace them.
A password is limited to 1023 encoded bytes. The facade does not implement
CPython's C extension ABI or supply Python threads/processes.

Unwrap feeds one TLS record at a time so following plaintext stays in the raw
TCP receive buffer. A successful nonblocking write retains unsent ciphertext;
subsequent operations flush it. Logging/callback failures can occur after TLS
has already consumed input or accepted output.

## Tests and removal

```sh
cargo test --locked -p twr-python-ssl-host --lib
just python-sdk
# Build components using the Docker command in the SDK README.
bash guests/python/test.sh
```

[Conformance](../../tests/conformance/README.md) preserves upstream test ASTs
and records platform/resource exclusions. BIO/socket/asyncio fixtures cover
local client/server exchanges, permissions, failures and cleanup independently.
Public test credentials are documented [here](../../tests/fixtures/ssl/README.md).

When upstream SSL passes these checks, omit `"ssl"` from SDK `install()` ports,
remove its WIT import, this port and its SDK manifest entry, then remove the
optional Cargo feature/dependency and core view. Other ports and the interpreter
remain independent.
