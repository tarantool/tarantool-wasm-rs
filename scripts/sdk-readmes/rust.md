# Rust SDK

Package: `tarantool-wasm` Cargo crate archive. Matching WIT: `../../wit/rust/`.

Dependencies:

- Rust 1.96.0+ and Cargo.
- cargo-component 0.21.1 and `wasm32-wasip1` for tarawasm's default Rust profile.
  Direct Cargo builds can use the `wasm32-wasip2` target.
- `wit-bindgen =0.58.0`.
- `serde 1.0` with `derive`, `rmp-serde 1.3` and `rmpv 1.3` with `with-serde`.
  Cargo resolves these dependencies from the packaged `Cargo.toml`.
- wasm-tools 1.261.0 for component stripping and validation.
- `tarantool:tarantool@0.2.0` (also included in the crate).

Installation and build instructions:
[tarawasm](https://github.com/mandesero/tarawasm).
