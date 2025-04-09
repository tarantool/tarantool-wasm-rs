use wasmtime::component::bindgen;
use crate::ffi;

bindgen!({
    path: "adder.wit",
    world: "adder",
    async: true,
    with: {
        "docs:adder/types/iterator": IteratorImpl,
        "docs:adder/tuple/tuple-iterator": TupleIteratorImpl,
    }
});

pub struct IteratorImpl {
    inner: tarantool::index::IndexIterator,
}

pub struct TupleIteratorImpl {
    iter: *mut ffi::types::BoxTupleIteratorT,
}

pub mod ttbox;
pub mod msgpack;
pub mod say;
pub mod error;
pub mod session;
pub mod txn;
pub mod types;
pub mod index;
pub mod sequence;
pub mod key_def;
pub mod tuple_format;
pub mod tuple;
