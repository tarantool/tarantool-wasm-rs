//! WebAssembly component bindings for Tarantool internals.
//!
//! The `bindgen!` macro below generates high level Rust interfaces for the
//! component world used by the runtime. Custom implementations are provided for
//! iterators so they can interact with Tarantool's native types.

use crate::ffi;
use wasmtime::component::bindgen;

bindgen!({
    world: "adder",
    imports: {
        default: async,
        "tarantool:tarantool/box-tuple.retain": async | trappable,
        "tarantool:tarantool/box-tuple.release": async | trappable,
        "tarantool:tarantool/box-tuple.field-count": async | trappable,
        "tarantool:tarantool/box-tuple.byte-size": async | trappable,
        "tarantool:tarantool/box-tuple.format": async | trappable,
        "tarantool:tarantool/box-tuple.compare": async | trappable,
        "tarantool:tarantool/box-tuple.[method]iterator.position": async | trappable,
        "tarantool:tarantool/box-tuple.[method]iterator.rewind": async | trappable,
        "tarantool:tarantool/key-def.release": async | trappable,
        "tarantool:tarantool/tuple-format.retain": async | trappable,
        "tarantool:tarantool/tuple-format.release": async | trappable,
    },
    exports: { default: async },
    with: {
        "docs:adder/lua-callback.handler": LuaCallbackHandler,
        "tarantool:tarantool/index.iterator": IndexIteratorImpl,
        "tarantool:tarantool/box-tuple.iterator": TupleIteratorImpl,
    }
});

/// Stable guest-side handle to a Lua callback registry entry.
pub struct LuaCallbackHandler {
    name: String,
}

/// Wrapper implementing the `iterator` interface for Tarantool's index
/// iterators.
pub struct IndexIteratorImpl {
    inner: Option<::tarantool::index::IndexIterator>,
}

/// Wrapper around a raw pointer implementing the `tuple-iterator` interface.
pub struct TupleIteratorImpl {
    iter: *mut ffi::types::BoxTupleIteratorT,
    tuple: *mut ffi::types::BoxTupleT,
}

pub mod error;
pub(crate) mod handles;
pub mod helpers;
pub mod index;
pub mod key_def;
pub mod lua_callback;
pub mod metrics;
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
