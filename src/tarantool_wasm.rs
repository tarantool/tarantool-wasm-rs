use wasmtime::component::{ResourceTable};
use wasmtime_wasi::bindings::Command;
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime::{Engine, Linker, Store, Module, TypedFunc, Instance};
use anyhow::Result;
use lazy_static::lazy_static;
use std::thread;
use std::collections::HashMap;
use std::sync::{Mutex, Arc};
use anyhow::anyhow;
use std::cell::RefCell;
use std::borrow::BorrowMut;
use wasmtime::component::Component;
use tokio::runtime::Runtime;
use wasmtime::Config;
use std::sync::mpsc;
use crate::tx::{tx_flush, tx_push, TxCtx};
use wasmtime::component::bindgen;
use crate::tarantool_wasm::docs::adder::tupler;


struct WasmCoro(minicoroutine::Coroutine<(), (),  std::result::Result<(), wasmtime::Error>, (), minicoroutine::GLOBAL>);
unsafe impl Send for WasmCoro{}


lazy_static! {
    static ref runtime_state: Arc<Mutex<RuntimeState>> = Arc::new(Mutex::new(RuntimeState::new()));
}

pub struct WasmStates {
    table: ResourceTable,
    ctx: WasiCtx,
}

struct SyncWasmStates(Mutex<WasmStates>);

bindgen!({
    path: "adder.wit",
    world: "adder",
    with: {
        "docs:adder/tupler/space": TtSpace,
    }
});


impl SyncWasmStates {
    pub fn new() -> Self {
        SyncWasmStates(Mutex::new(WasmStates::new()))
    }
}

impl WasmStates {
    pub fn new() -> Self {
        let table = ResourceTable::new();
        let mut builder = WasiCtxBuilder::new();
        // TODO: make env and stdio configurable
        builder.inherit_stdio().inherit_env();
        let ctx = builder.build();
        Self { table, ctx }
    }
}

impl WasiView for SyncWasmStates {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.0.get_mut().unwrap().table
    }

    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.0.get_mut().unwrap().ctx
    }
}

impl tupler::Host for SyncWasmStates {
    fn get_tuple(&mut self, name: String, value: Vec<u8>) -> tupler::TtTuple {
        tupler::TtTuple {
            name: name,
            value: value,
            status: true,
        }
    }
}

pub struct TtSpace {
    space_id: u32
}

impl tupler::HostSpace for SyncWasmStates {
    fn new(&mut self, name: String) -> wasmtime::component::Resource<TtSpace> {
        let space_id = if name == "test" { 512 }  else  { 0 };
        self.0.lock().unwrap().table.push(TtSpace{space_id}).unwrap()
    }

    fn insert(&mut self, _self_: wasmtime::component::Resource<TtSpace>, tup: tupler::TtTuple) -> Result<tupler::TtTuple, String> {
        minicoroutine::Coroutine::<(), (),  std::result::Result<(), wasmtime::Error>, (), minicoroutine::GLOBAL>::running().unwrap().yield_(());
        println!("TX");
        minicoroutine::Coroutine::<(), (),  std::result::Result<(), wasmtime::Error>, (), minicoroutine::GLOBAL>::running().unwrap().yield_(());
        Ok(tup)
    }

    fn drop(&mut self, self_: wasmtime::component::Resource<TtSpace>) -> Result<(), wasmtime::Error> {
        self.0.lock().unwrap().table.delete(self_);
        Ok(())
    }
}

struct RuntimeState {
    pub engine: Engine,
    pub linker: wasmtime::component::Linker<SyncWasmStates>,
    pub modules: HashMap<String, Component>
}

impl RuntimeState {
    pub fn new() -> Self {
        let mut config = Config::new();
        config.async_support(true);
        let engine = Engine::new(&config).unwrap();
        let mut linker = wasmtime::component::Linker::<SyncWasmStates>::new(&engine);
        wasmtime_wasi::add_to_linker_async(&mut linker);
        tupler::add_to_linker(&mut linker, |state| state);
        let modules = HashMap::new();
        RuntimeState { engine, linker, modules }
    }
}

pub fn tt_wasm_load_module(path: String) -> Result<String> {
    let mut state = runtime_state.lock().unwrap();
    match Component::from_file(&state.engine, path.clone()) {
        Ok(module) => {
            state.modules.insert(path.clone(), module);
            return Ok(path);
        },
        Err(e) => return Err(e),
    }
}

pub fn tt_wasm_run_module(module_uid: String) -> Result<()> {
    let state = runtime_state.lock().unwrap();
    let engine = state.engine.clone();
    let component = state.modules.get(&module_uid).unwrap().clone();
    let linker = state.linker.clone();
    let wasi_view = SyncWasmStates::new();
    let mut store = Store::new(&engine, wasi_view);
    let store_cell = RefCell::new(store);
    drop(state);


    let handle = thread::Builder::new().name("MyThread".to_string()).spawn(move || {
        let rt = Runtime::new().unwrap();
        let (ret_tx, ret_rx) = mpsc::channel();
        let mut coroutine = WasmCoro(minicoroutine::Coroutine::new(move |_| {
            rt.block_on(async {
                let mut store = store_cell.borrow_mut();
                let command = Command::instantiate_async(&mut *store, &component, &linker);
                command.await.unwrap().wasi_cli_run().call_run(&mut *store).await;
                let current_thread = thread::current();
                println!("Current thread name: {:?}", current_thread.name());
            });
            Ok(())
        }, ()).unwrap());

        let (tx, rx) = mpsc::channel();
        println!("Running");
        coroutine.0.resume(());
        println!("Yielded");
        println!("Sending");
        tx.send(coroutine);
        println!("Sended");
        let cb = move || -> () {
            let mut coro: WasmCoro = rx.recv().unwrap();
            println!("resuming");
            coro.0.resume(());
            println!("Yielded");
            ret_tx.send(coro);
            println!("Sended back");
        };
        tx_push(
            Box::new(TxCtx{cb: Box::new(cb)})
        );
        tx_flush();
        Ok::<(), wasmtime::Error>(())
    });
    Ok(())
}
