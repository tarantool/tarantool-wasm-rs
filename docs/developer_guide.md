# Developer Guide

This document explains the internal structure of the project, the main APIs and the recommended workflow for contributors.

## Repository structure

- `src/` – Rust sources of the Tarantool WASM library.
- `src/bindings/` – implementation of the `tarantool:tarantool` WIT interfaces.
- `src/runtime/` – runtime helpers such as the executor and TX thread integration.
- `docs/` – documentation. The user facing `wasm_component.md` and this guide live here.
- `test/` – Lua tests executed via `luatest`.
- `scripts/` – helper scripts used by `just` tasks.
- `third-party/` – git submodules with patched Tarantool and WASI TLS.
- `wit/` – local WIT interface files used by the examples and tests.
- `justfile` – automation tasks for building the project and running tests.
- `wkg.toml` / `wkg.lock` – configuration for the `wkg` package manager used to fetch WIT dependencies.

## Internal APIs

The library exposes a Lua module named `wasm` which is built from `src/lib.rs`. The module provides several functions:

- `load(path[, opts]) -> string` – load a WASM component from file and return a module identifier. Optional `opts`
  configures the WASI context for subsequent runs.
- `run(uid[, opts]) -> join_handle` – run a component using `wasi:cli.run` entry point in a background
  thread. `opts` overrides the configuration passed to `load`.
- `join(handle)` – wait for completion of a module started with `run`.
- `call(uid, func, ...) -> any` – call an exported function synchronously. Arguments and results are encoded with MessagePack.
- `batch(uid, calls) -> table` – execute multiple exports sequentially inside one worker thread.

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
- `preopened_dirs` (default: none) – array of `{ host_path, guest_path, perms }` tables exposing directories to the module.
  - `perms` is optional and can be `"ro"` (read-only) or `"rw"` (read-write, default).

Providing `stdin`, `stdout`, or `stderr` overrides the respective `inherit_*` option and redirects the module's streams to files.

The heavy lifting happens in `src/runtime/`:

- `tarantool_wasm.rs` – initializes the Wasmtime engine, links Tarantool bindings and manages loaded components. It also spawns worker threads and handles graceful shutdown.
- `wasm_fiber.rs` – tiny wrapper around a Tokio runtime running inside a separate OS thread. It allows yielding execution back to the Tarantool transaction thread.
- `executor.rs` – abstraction over task execution. Depending on the state it either runs tasks directly or forwards them to the TX thread via `tx_push`/`tx_flush`.
- `convert.rs` – utilities to convert between Wasmtime component values, MessagePack and Lua values.
- `tx.rs` – FFI bindings used to schedule callbacks on the Tarantool transaction thread.

Bindings under `src/bindings/` provide the implementation of WIT interfaces for database access (`ttbox`, `msgpack`, `txn`, etc.) and utility modules (`zlib`). They rely heavily on the `tarantool` Rust crate and FFI definitions from `src/ffi.rs`.

## Setting up the environment

1. Ensure the Rust toolchain is installed.
2. Install `wkg` and `just` if they are not available:

   ```sh
   cargo install wkg
   cargo install just
   ```

3. Fetch submodules, patch the `wkg` config and build everything:

   ```sh
   just
   ```

4. The resulting shared library will appear in `target/debug/libtarantool_wasm_rs.so` and can be placed into `.rocks/lib/tarantool/wasm.so` for testing.

## Running tests

Tests require a Tarantool build with WASM support. The workflow is automated in the CI and can be reproduced locally:

```sh
just build-tarantool   # build patched Tarantool under third-party/
just install-luatest   # one time installation of the testing framework
just test              # run the Lua test suite
```

## Contribution guidelines

- Format Rust code with `cargo fmt -- --check` before committing.
- Ensure `just test` passes locally.
