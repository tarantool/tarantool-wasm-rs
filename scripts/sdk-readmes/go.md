# Go SDK

Package: `twr-go-sdk` module archive. Matching WIT: `../../wit/go/`.

Dependencies:

- Go 1.27.2 and TinyGo 0.43.0 for the verified component toolchain.
- `go.bytecodealliance.org/cm v0.3.0` (recorded in the module's `go.mod`).
- wit-bindgen-go 0.7.0, Binaryen 133 and wasm-tools 1.261.0.
- `tarantool:tarantool@0.2.0`, WASI CLI/sockets 0.2.12 and optional
  `docs:adder/lua-callback@0.1.3` / `docs:adder/zlib@0.1.3` (supplied WIT).

Synchronous database builds use `-scheduler=none -panic-unwind=explicit`.
TinyGo 0.42 does not provide the SDK's panic cleanup guarantees.

Installation and build instructions:
[tarawasm](https://github.com/mandesero/tarawasm).
