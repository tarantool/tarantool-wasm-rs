//! Support code for executing WebAssembly modules within Tarantool.
//!
//! This module houses conversion utilities, executors and high level wrappers
//! around the Wasmtime runtime used by the public Lua interface.

pub mod convert;
pub mod executor;
pub(crate) mod lifecycle;
pub(crate) mod metrics;
pub mod tarantool_wasm;
pub mod tx;
pub mod wasm_fiber;
