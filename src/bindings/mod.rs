use crate::ffi;
use wasmtime::component::bindgen;

bindgen!({
    world: "adder",
    async: true,
    with: {
        "tarantool:tarantool/types/iterator": IteratorImpl,
        "tarantool:tarantool/box-tuple/tuple-iterator": TupleIteratorImpl,
    }
});

pub struct IteratorImpl {
    inner: ::tarantool::index::IndexIterator,
}

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

pub mod wlua;
pub mod zlib;
