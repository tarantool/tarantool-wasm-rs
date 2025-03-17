use wasmtime::component::bindgen;

bindgen!({
    path: "adder.wit",
    world: "adder",
    async: true,
});

pub mod ttbox;
pub mod msgpack;
pub mod say;
pub mod error;
pub mod session;
pub mod txn;
