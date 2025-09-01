//! WebAssembly component bindings for Tarantool internals.
//!
//! The `bindgen!` macro below generates high level Rust interfaces for the
//! component world used by the runtime. Custom implementations are provided for
//! iterators so they can interact with Tarantool's native types.

use crate::ffi;
use wasmtime::component::bindgen;

bindgen!({
    world: "adder",
    async: true,
    with: {
        "tarantool:tarantool/index/index-iterator": IndexIteratorImpl,
        "tarantool:tarantool/box-tuple/tuple-iterator": TupleIteratorImpl,
    }
});

/// Wrapper implementing the `iterator` interface for Tarantool's index
/// iterators.
pub struct IndexIteratorImpl {
    inner: ::tarantool::index::IndexIterator,
}

/// Wrapper around a raw pointer implementing the `tuple-iterator` interface.
pub struct TupleIteratorImpl {
    iter: *mut ffi::types::BoxTupleIteratorT,
}

pub mod error;
pub mod index;
pub mod key_def;
pub mod msgpack;
pub mod say;
pub mod sequence;
pub mod session;
pub mod ttbox;
pub mod tuple;
pub mod tuple_format;
pub mod txn;
pub mod types;

pub mod zlib;
