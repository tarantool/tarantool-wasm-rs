# Go SDK for Tarantool-Wasm-Rs

This module provides an idiomatic Go API for `tarantool:tarantool@0.2.0`.
Database operations and optional Lua callbacks accept ordinary Go values. The
SDK encodes MessagePack directly and releases temporary tuples. It runs inside
Tarantool-Wasm-Rs; it is not a network client for standalone Go applications.
The SDK covers spaces and indexes, CRUD and iteration, tuples and key definitions,
transactions, sequences, sessions and iproto, diagnostics, logging and metrics.
It includes raw Tarantool bindings, optional Lua callbacks and a WASI network
adapter for TinyGo's `net` and `net/http`. Zlib is available
only as raw WIT API. The repository license applies.

## Install the archive

Use `dist/twr-go-sdk-0.1.0.tar.gz` from Go SDK CI, or package the module with
`bash guests/go/tests/pack.sh`.
Extract it beside your project and add:

```sh
go mod edit -require=github.com/tarantool/tarantool-wasm-rs/guests/go@v0.1.0
go mod edit -replace=github.com/tarantool/tarantool-wasm-rs/guests/go=../twr-go-sdk-0.1.0
go mod tidy
```

```go
import twr "github.com/tarantool/tarantool-wasm-rs/guests/go"

func CreateUser(id uint64, name string) ([]any, error) {
    return twr.Box.Space("users").Insert([]any{id, name})
}
```

Spaces and indexes accept names or numeric IDs. For example,
`Box.Space(uint32(512)).Index("primary").Get(uint64(1))`.
Names resolve on every operation, including after a schema change. Space, index,
sequence and session identifiers are borrowed catalog IDs, with no `Close`.
The SDK cannot create schema objects because WIT provides no such functions.
Imports, `init`, `Box`, and construction of schema proxies make no host calls.

## Your own world and build

Use your own package, world and exports; neither `world adder` nor application
`internal` packages are dependencies of the installed SDK. Wire application
exports in `main.go` to your own generated bindings; see `tests/components/base`.

```wit
package example:go-base@0.1.0;
world sdk-test {
  include wasi:cli/imports@0.2.12;
  include tarantool:tarantool/guest@0.2.0;
  export check: func(name: string) -> string;
  export roundtrip: func(data: list<u8>) -> list<u8>;
  export wasi:cli/run@0.2.12;
}
```

TinyGo requires the complete `wasi:cli/imports` world, even though linking removes
unused runtime imports. Use the repository's current WASI dependencies (0.2.12)
and retain WIT locks and `go.sum`. The base component has no callback, host zlib, socket or
TLS imports. Optional imports belong only in the worlds that use them.

Verified toolchain: Go 1.27.2, TinyGo 0.43.0, wit-bindgen-go 0.7.0,
wasm-tools 1.261.0, wkg 0.16.1 and Binaryen 133. Tarawasm: **latest**, checked out from Git.
Use TinyGo 0.43.0 for panic cleanup; 0.42.0 does not unwind Wasm panics.

Tarawasm installs TinyGo 0.43 and its matching Binaryen through its managed Go
toolchain. From a Git checkout of tarawasm (selected by `PYTHONPATH`), run:

```sh
python -m tarawasm.cli toolchain setup go --mode local --install --yes --upgrade
python -m tarawasm.cli doctor --lang go --json
```

`--upgrade` selects the recommended versions even if TinyGo 0.42 is installed.
For the direct commands below, put Go, TinyGo, Binaryen (`wasm-opt`), wkg and
wasm-tools on PATH. Tarawasm's `managed_environment()` provides these paths.
Then use the existing tarawasm commands from the application directory:

```sh
wkg fetch --type wasm
python -m tarawasm.cli bind --execution local -- --versioned
python -m tarawasm.cli build --execution local -- -scheduler=none -panic-unwind=explicit
python -m tarawasm.cli strip --execution local sdk.wasm
mv sdk.strip.wasm sdk.wasm
wasm-tools validate sdk.wasm
```

The fixture `tarawasm.json` selects language `go`, source `.`, its WIT world and
`sdk.wasm`. Build flags are necessary: explicit panic unwinding is incompatible
with TinyGo's asyncify scheduler. This synchronous SDK does not use goroutines.
Equivalent direct build from the same fixture:

```sh
go tool wit-bindgen-go generate --versioned --world sdk-test -o internal wit
tinygo build -target=wasip2 -scheduler=none -panic-unwind=explicit \
  --wit-package wit --wit-world sdk-test -o direct.wasm .
wasm-tools strip direct.wasm -o direct.wasm
wasm-tools validate direct.wasm
```

`tarawasm bind` records wit-bindgen-go as a Go tool. For a new direct-only project,
first run `go get -tool go.bytecodealliance.org/cmd/wit-bindgen-go@v0.7.0` and
`go get go.bytecodealliance.org@v0.7.0`. Generated application bindings remain
application-owned. SDK raw bindings are shipped separately with fixed imports.
TinyGo uses the `tinygo.wasm` tag; WASI target architecture is not Go's `wasm` tag.

When starting an application with `tarawasm init --lang go`, its Go imports use
the default unversioned binding paths. The `bind -- --versioned` command above
adds version directories to those paths. Update application-owned imports to
match before building; tarawasm does not rewrite your implementation. For the
world above, the world bindings are under
`<your-module>/internal/example/go-base/v0.1.0/sdk-test`. The SDK fixtures already
use the matching versioned paths.

## Values and MessagePack

`Pack` and `Unpack` also work in native Go. They never use JSON intermediates.
`FromJSON`/`ToJSON` explicitly expose WIT's JSON conversion functions only.

* `[]byte` encodes binary. Other slices/arrays encode arrays; strings require UTF-8.
* Use `int64` and `uint64` for 64-bit values, including both full boundary ranges.
  Smaller integer types also work. Unsigned wire integers decode as `uint64`;
  signed wire integers as `int64`. Nonnegative signed inputs use compact unsigned
  encodings, so integer value is preserved but original Go signedness is not.
* `nil`, nil pointers, interfaces containing typed nil, nil slices and nil maps
  encode nil. Non-nil empty slices/maps encode empty containers and decode non-nil.
* String-key maps decode as `map[string]any`. Other comparable-key maps decode
  as `map[any]any`. `codec.Pairs` supports array/binary keys, duplicate keys and
  map order. Map keys that cannot be represented safely in Go decode as `Pairs`.
* `codec.UUID`, `Decimal`, `Datetime`, `Interval`, and `ErrorValue` represent
  Tarantool extensions. Decimal keeps digits, scale and sign, never floating
  point. Datetime keeps signed seconds, nanoseconds, TZ offset and TZ index;
  there is no implicit `time.Time` conversion. Error values preserve stacks and
  future numeric attributes. Unknown extensions preserve signed type and bytes.
* Unsupported structs, channels/functions and cyclic/excessively nested values
  return clear errors. The depth limit is 128. Invalid/trailing MessagePack is
  rejected. Values are copied from host buffers. Host-supported extension ranges
  can be narrower than the representable wire format and return a host error.

Tuple/key/operation fields in the public API are **zero-based**. Negative update
positions retain Tarantool semantics. `Operation{Operator:"=", Field:1,
Arguments:[]any{"new name"}}` replaces the second field. The SDK adjusts tuple
update numbering to its host C API. Raw WIT calls retain their native numbering.
Numeric tuple path selectors also start at zero: `[1]` addresses the second field.
A missing CRUD result is `nil, nil`. `Tuple.Field`/cursor reads return a separate
`present` boolean to distinguish absence from an actual nil field.
`Select` returns a non-nil empty slice for no rows. Scalar keys are wrapped in a
single-element array; nil scan keys mean an empty key.

## Resources and errors

Ordinary CRUD returns values and closes the temporary tuple on every path.
For explicit handles, use `Box.NewTuple`, `NewKeyDef`, `NewTupleFormat`, and
`DefaultTupleFormat`. Each returned object owns one reference. Use `defer Close`.
Tuple/format `Copy` retains another reference; key definition `Copy` duplicates.
Borrowed arguments are used only for the call. Tuple cursors independently retain
their tuple and remain usable after the original tuple is closed.

Go value copies share a pointer to ownership state: closing any alias closes that
owner exactly once. `Close` is idempotent. Operations after closure return
`ErrClosed`. GC/finalizers do not manage correctness. `Using(resource, fn)` closes
on success, error or panic; cleanup errors join the original error and never
replace a primary panic. Fatal Wasm traps/OOM cannot execute Go defers.
The helpers also preserve `panic(nil)`, for which TinyGo's `recover` returns nil.

Explicit index iterators require `defer it.Close()`. `Next` closes on exhaustion
or error, and after the last requested row when a limit is set. `Iter`, `Each`
and `Select` all apply `ScanOptions.Offset` and `Limit`; zero limit returns no rows.
Prefer `Space.Each`/`Index.Each`: returning `false` stops early and closes
the iterator, including callback error/panic paths. `Tuple.Each` behaves similarly.
`TupleCursor.Read` preserves an exhausted cursor for rewind; `Next` closes it.
Go range-function iteration is not used or promised by this toolchain.

Tarantool `*twr.Error` implements `error` and preserves code, message, host type,
optional file and optional line. `errors.As` extracts it; `errors.Is` matches a
numeric code and an optional type. `ErrClosed`, `ErrNotFound` and transaction
sentinels also work with `errors.Is`. Diagnostics use `LastError`, `SetError`,
`ClearError`, `NewError`, and `FormatError`.

## Lua callbacks and raw API

Add `docs:adder/lua-callback@0.1.3` to that application's world. In `wkg.toml`,
set `[overrides]` with `"docs:adder" = { path = "../twr-go-sdk-0.1.0/wit/runtime" }`.
`wkg fetch` installs it into one dependency directory.
This single `docs:adder@0.1.3` package contains both callback and zlib interfaces;
versions and ABI match the runtime. Import only what your component uses.

```go
import "github.com/tarantool/tarantool-wasm-rs/guests/go/luacallback"

err := luacallback.Using("my_callback", func(c *luacallback.Callback) error {
    result, err := c.Call([]any{uint64(1), "hello"})
    _ = result
    return err
})
```

`Open` returns an owned callback; explicit users defer `Close`. Callback failures
are `*luacallback.Error`. Lua registers/replaces names using
`wasm.register_callback(name, fn)` and unregisters them explicitly. The SDK never
registers callbacks. An open handle resolves the name dynamically on each call.

`raw/tarantool/tarantool/<interface>` exposes original generated WIT functions,
records, enums and canonical ABI `cm.List`, `cm.Option`, `cm.Result` values without
argument conversion. Record handles returned by raw APIs must be released with
the corresponding WIT release; raw iterators/callbacks use `ResourceDrop`.
`raw/docs/adder/zlib` exposes only `Compress`, `Decompress`, `Crc32`, `Adler32`.
For native unit tests, inject `bindings.Caller` into `New` or use
`luacallback.OpenWithBindings`. Native raw host calls panic because no host exists.

## Transactions and runtime restrictions

`Box.Transaction(func() error { ... }, TransactionOptions{})` commits on success,
rolls back on error/panic and preserves the original panic. Rollback errors are
joined with the original error. Nested transactions and manual completion through
that client's helper are rejected. IDs detect completion/replacement via raw APIs
or another client; a replacement transaction is left to its owner to roll back.
Use one client and synchronous execution; do not use goroutines within a
transaction. WIT exposes transaction IDs but no goroutine identity, so it cannot
detect every concurrent access. `-scheduler=none` enforces the tested synchronous
execution model. Coroutine schedulers are not supported by this SDK's guarantees.

Export `wasi:cli/run@0.2.12` and use `wasm.join(wasm.run(module))` for transactions.
`wasm.call` cannot begin a transaction: the host requires a reschedulable run.
Memtx data transactions need `memtx_use_mvcc_engine=true` because host calls yield;
Vinyl retains its runtime semantics. Applications must configure MVCC themselves.
The SDK never changes runtime policy, network permissions or database settings.

## Optional networking

TinyGo 0.43.0 needs a driver to connect `net` to WASI sockets. Enable the SDK's
optional driver explicitly before the first network operation:

```go
import (
    "io"
    "net/http"
    "time"

    "github.com/tarantool/tarantool-wasm-rs/guests/go/network"
)

func Fetch(url string) ([]byte, error) {
    if err := network.Enable(network.Options{Timeout: 2 * time.Second}); err != nil {
        return nil, err
    }
    response, err := http.Get(url)
    if err != nil {
        return nil, err
    }
    defer response.Body.Close()
    return io.ReadAll(response.Body)
}
```

The application world must include `wasi:cli/imports@0.2.12`, as in
`tests/components/probes`. Its socket imports provide name resolution and outgoing
TCP. The SDK ships its own bindings; application-generated `internal` packages
are not dependencies. Importing `network` and calling `Enable` create no host
resources. Repeating the same configuration is safe; changing it is rejected.
Without `Enable`, TinyGo retains its `Netdev not set` placeholder.

For `net/http`, use TinyGo's default **asyncify** scheduler. Build the network
fixture with either build recipe above, omitting
`-scheduler=none -panic-unwind=explicit`.

Asyncify and explicit panic unwinding are incompatible in this toolchain.
The database SDK's panic cleanup and transaction guarantees require its synchronous
build mode; do not rely on those guarantees in an asyncify network component.

The adapter supports `net.LookupHost`, outgoing TCP reads/writes, deadlines set
before an operation, `Close`, keepalive options, and plain HTTP using Go's transport. DNS returns
the first address because TinyGo's netdev hook accepts one address. It does not
support UDP, listening sockets, scoped IPv6 addresses or TLS. Native Go does not
need this driver; `Enable` there returns `network.ErrUnsupported`.
The hook is tied to the verified TinyGo version and must be rechecked on upgrades.

`Options.Timeout` bounds each DNS lookup and TCP connection attempt (default five
seconds). TinyGo's hook does not forward `net.Dialer` contexts/deadlines, nor
deadline changes made during a pending read/write. Read/write timeouts can be
identified with `errors.As` into `interface{ Timeout() bool }`; TinyGo's
`net.OpError` lacks the deprecated `Temporary` method required by `net.Error`.
TinyGo 0.43's `http.Client.Timeout` and request cancellation do not bound HTTP
I/O. A server that accepts TCP but never responds can leave `Get` waiting
indefinitely. Some HTTP transport
error paths also leave the underlying connection open until `wasm.drop`; success
requires `response.Body.Close`. Applications that need bounded HTTP I/O and
reliable connection cleanup on transport errors cannot rely on this TinyGo
transport.

Host permissions still apply: `inherit_network`, `allow_tcp`,
`allow_ip_name_lookup`, `allowed_ips` and `allowed_ports`. The SDK never changes
them. A denied DNS lookup returns the host's `permanent-resolver-failure`; denied
TCP/IP/port access returns `access-denied`.

## Compatibility

Go's `compress/flate`, `compress/gzip`, `compress/zlib`, `hash/crc32` and
`hash/adler32` work in guest memory with Go stream and error semantics. The raw
host zlib API instead copies WIT byte buffers and uses the host's libz.
DNS, TCP and HTTP work with the optional driver and emit WASI socket imports.
IPv6 TCP is unverified; external HTTPS checks are skipped. TLS, UDP and listening
sockets are unsupported. See `sdk-manifest.json` for compiled/runtime results,
verified platforms and tool versions. Python and JavaScript are used only by
compatibility tests and are not SDK runtime dependencies.
