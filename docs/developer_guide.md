# Developer Guide

This document explains the internal structure of the project, the main APIs and the recommended workflow for contributors.

## Repository structure

- `src/` – Rust sources of the Tarantool WASM library.
- `src/bindings/` – implementation of the `tarantool:tarantool` WIT interfaces.
- `src/runtime/` – runtime helpers such as the executor and TX thread integration.
- `docs/` – documentation. The user facing `wasm_component.md` and this guide live here.
- `test/` – Lua tests executed via `luatest`.
- `scripts/` – helper scripts used by `just` tasks.
- `third-party/` – git submodules with WASI TLS.
- `wit/` – local WIT interface files used by the examples and tests.
- `justfile` – automation tasks for building the project and running tests.
- `wkg.toml` / `wkg.lock` – configuration for the `wkg` package manager used to fetch WIT dependencies.

## Internal APIs

The library exposes a Lua module named `wasm` which is built from `src/lib.rs`. The module provides several functions:

- `load(path[, opts]) -> string` – load a WASM component from file and return a module identifier. Optional `opts`
  configures the WASI context for subsequent runs.
- `precompile(input, output[, opts]) -> bool` – create a native component using
  the runtime's exact Wasmtime configuration.
- `run(uid[, opts]) -> join_handle` – run a component using the `wasi:cli/run` entry point in a background
  thread. When present, `opts` replaces the complete configuration passed to `load`; it is not merged field by field.
- `cancel(handle) -> bool` – request cancellation of a task started with `run` and wait for the worker
  thread to terminate.
- `join(handle)` – wait for completion of a module started with `run`.
- `call(uid, func, ...) -> any` – call an exported function synchronously. Arguments and results are encoded with MessagePack.
- `batch(uid, calls) -> table` – execute multiple exports sequentially inside one worker thread. Each item gets a new
  store and component instance; use a session when guest state must persist between calls.
- `interpreter_exec(uid, code) -> bool` – execute source in shared interpreter mode.
- `interpreter_call(uid, func, ...) -> any` – call a function in shared interpreter mode.
- `open_session(uid[, opts]) -> string` – open a long-lived session with a persistent component instance. As with `run`, `opts` replaces the complete load configuration.
- `session_call(session, func, ...) -> any` – call an export using an opened session.
- `session_batch(session, calls) -> table` – execute multiple calls sequentially in the same session.
- `close_session(session) -> bool` – close a previously opened session.
- `exports(uid) -> table` – return a map of exported function names to their parameter and result types.
- `bind(uid) -> table` – return a Lua proxy whose functions call the component's root and interface exports.
- `drop(uid) -> bool` – unload a module and free associated resources.

`opts` is a table configuring the WASI environment:

- `inherit_env` (default: `true`) – pass the host environment variables to the module.
- `env`(default: none) – table of additional environment variables to set.
- `inherit_args` (default: `false`) – append the host process arguments.
- `args` – array of arguments to pass to the module.
- `inherit_stdin` (default: `true`) – forward the host's standard input stream.
- `inherit_stdout` (default: `true`) – forward the host's standard output stream.
- `inherit_stderr` (default: `true`) – forward the host's standard error stream.
- `stdin` (default: none) – path to a file used as the module's standard input.
- `stdout` (default: none) – path to a file where the module's standard output is written.
- `stderr` (default: none) – path to a file where the module's standard error is written.
- `inherit_network` (default: `true`) – allow the module to access host networking.
- `allow_ip_name_lookup` (default: `true`) – enable DNS lookups.
- `allow_tcp` / `allow_udp` (default: `true`) – enable TCP/UDP sockets.
- `memory_limit` (default: none) – maximum size of WebAssembly linear memory in bytes.
- `max_instructions` (default: none) – maximum fuel (approximate instruction count) the module may consume.
- `interpreter` (default: `false`) – enable shared interpreter singleton mode for the module path.
- `precompiled` (default: `false`) – deserialize a trusted native artifact
  produced by `wasm.precompile`. Never enable this for untrusted input.
- `allowed_ips` (default: none) – array of IP addresses permitted for network access.
- `allowed_ports` (default: none) – array of TCP/UDP ports permitted for network access.
- `preopened_dirs` (default: none) – array of `{ host_path, guest_path, perms = "ro" | "rw" }` tables exposing directories to the module. The host and guest paths are positional entries.
  - `perms` is optional and can be `"ro"` (read-only) or `"rw"` (read-write, default).

Defaults are applied only when a field is absent. Invalid field types, malformed IP addresses, unknown permission values,
and inaccessible stream or preopened-directory paths are rejected with a Lua error.

Providing `stdin`, `stdout`, or `stderr` overrides the respective `inherit_*` option and redirects the module's streams to files.

Fuel support is selected when the component is loaded. To use `max_instructions`
in `run` or `open_session` overrides, load the component with
`max_instructions` set as well; otherwise the call is rejected.

Shared interpreters are keyed by canonical component path, not by Tarantool
session. Their state is therefore visible to every client using the same path.
Ordinary guest errors preserve the instance; the worker reinstantiates the
component only after a WASM trap, discarding its state. See
`python-interpreter/README.md` for the pinned toolchain and command used to
rebuild the Python fixture.

Wasmtime's persistent compilation cache is enabled for ordinary `.wasm`
components. For the lowest startup latency, precompile once and load the native
artifact explicitly:

```lua
wasm.precompile('python.wasm', 'python.cwasm')
local python = wasm.load('python.cwasm', { precompiled = true })
```

If fuel is required, pass `max_instructions` to both calls. Precompiled
artifacts are platform- and Wasmtime-version-specific and contain native
executable code, so they must be distributed as trusted binaries.

The heavy lifting happens in `src/runtime/`:

- `tarantool_wasm.rs` – façade that exposes the runtime API and records the Tarantool TX thread.
- `tarantool_wasm/registry.rs` – loaded-module registry and compiled component cache.
- `tarantool_wasm/store.rs` – WASI configuration and `Store` construction.
- `tarantool_wasm/execution.rs` – one-shot calls, batches, and background runs.
- `tarantool_wasm/session.rs` – persistent component sessions and their workers.
- `tarantool_wasm/interpreter.rs` – shared interpreter lifecycle and command workers.
- `tarantool_wasm/shutdown.rs` – deterministic shutdown of sessions, interpreters, and background runs.
- `wasm_fiber.rs` – tiny wrapper around a Tokio runtime running inside a separate OS thread. It allows yielding execution back to the Tarantool transaction thread.
- `executor.rs` – abstraction over task execution. Depending on the state it either runs tasks directly or forwards them to the TX thread via `tx_push`/`tx_flush`.
- `convert.rs` – utilities to convert between Wasmtime component values, MessagePack and Lua values.
- `lifecycle.rs` – process-wide `Running` → `Stopping` → `Stopped` state and active worker accounting.
- `tx.rs` – schedules callbacks on the Tarantool transaction thread. During shutdown, callbacks from existing workers are diverted to an in-process queue drained by TX.

Bindings under `src/bindings/` provide the implementation of WIT interfaces for database access (`ttbox`, `msgpack`, `txn`, etc.) and utility modules (`zlib`). They rely heavily on the `tarantool` Rust crate and FFI definitions from `src/ffi.rs`.

## Setting up the environment

1. Install Rust 1.94 or newer, as declared by `rust-version` in `Cargo.toml`.
2. Install `wkg` and `just` if they are not available:

   ```sh
   cargo install just
   just install-wkg
   ```

3. Fetch submodules, patch the `wkg` config and build everything:

   ```sh
   just
   ```

4. The resulting shared library appears in `target/debug/libtarantool_wasm_rs.so`.
   Put it where Tarantool's module loader used by the tests expects it:

   ```sh
   mkdir -p .rocks/lib/tarantool
   cp target/debug/libtarantool_wasm_rs.so .rocks/lib/tarantool/wasm.so
   ```

## Running tests

Tests require a Tarantool binary, `tt`, and the shared library at
`.rocks/lib/tarantool/wasm.so`. After building and copying the library as shown
above, install the test framework and run the suite:

```sh
just install-luatest   # one time installation of the testing framework
just test              # run the Lua test suite
```

## Contribution guidelines

- Format Rust code with `cargo fmt -- --check` before committing.
- Ensure `just test` passes locally.
