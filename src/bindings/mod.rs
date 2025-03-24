use wasmtime::component::bindgen;
use crate::ffi;

bindgen!({
    path: "adder.wit",
    world: "adder",
    async: true,
    with: {
        "docs:adder/types/iterator": IteratorImpl,
    }
});

pub struct IteratorImpl {
    iter: *mut ffi::types::BoxIteratorT,
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
