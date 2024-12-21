use crate::tarantool_wasm::SyncWasmStates;
use std::thread;
use wasmtime::component::bindgen;
use wasmtime_wasi::async_trait;
use crate::bindings::docs::adder::tupler;


bindgen!({
    path: "adder.wit",
    world: "adder",
    async: true,
    with: {
        "docs:adder/tupler/space": TtSpace,
    }
});

pub struct TtSpace {
    space_id: u32
}

#[async_trait]
impl tupler::Host for SyncWasmStates {
    async fn get_tuple(&mut self, name: String, value: Vec<u8>) -> tupler::TtTuple {
        let current_thread = thread::current();
        println!("Current thread name: {:?}", current_thread.name());
        self.yield_now().await;
        let current_thread = thread::current();
        println!("Current thread name: {:?}", current_thread.name());

        tupler::TtTuple {
            name: name,
            value: value,
            status: true,
        }
    }
}

#[async_trait]
impl tupler::HostSpace for SyncWasmStates {
    async fn new(&mut self, name: String) -> wasmtime::component::Resource<TtSpace> {
        let space_id = if name == "test" { 512 }  else  { 0 };
        self.0.lock().unwrap().table.push(TtSpace{space_id}).unwrap()
    }

    async fn insert(&mut self, _self_: wasmtime::component::Resource<TtSpace>, tup: tupler::TtTuple) -> Result<tupler::TtTuple, String> {

        println!("TX");
        let current_thread = thread::current();
        println!("Current thread name: {:?}", current_thread.name());
        self.yield_now().await;
        let current_thread = thread::current();
        println!("Current thread name: {:?}", current_thread.name());
        Ok(tup)
    }

    async fn drop(&mut self, self_: wasmtime::component::Resource<TtSpace>) -> Result<(), wasmtime::Error> {
        self.0.lock().unwrap().table.delete(self_);
        Ok(())
    }
}
