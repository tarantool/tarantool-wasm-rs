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

struct WasmCoro(minicoroutine::Coroutine<(), (),  std::result::Result<(), wasmtime::Error>, (), minicoroutine::GLOBAL>);
//unsafe impl Send for WasmCoro{}


lazy_static! {
    static ref runtime_state: Arc<Mutex<RuntimeState>> = Arc::new(Mutex::new(RuntimeState::new()));
}

pub struct WasmStates {
    table: ResourceTable,
    ctx: WasiCtx,
}

struct SyncWasmStates(Mutex<WasmStates>);

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

impl SyncWasmStates {
    pub fn new() -> Self {
        SyncWasmStates(Mutex::new(WasmStates::new()))
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
        let mut coroutine = WasmCoro(minicoroutine::Coroutine::new(move |_| {
            let mut store = store_cell.borrow_mut();
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let command = Command::instantiate_async(&mut *store, &component, &linker);
                command.await.unwrap().wasi_cli_run().call_run(&mut *store).await;
                let current_thread = thread::current();
                println!("Current thread name: {:?}", current_thread.name());
            });
            Ok(())
        }, ()).unwrap());
        coroutine.0.resume(());
        Ok::<(), wasmtime::Error>(())
    });
    handle.unwrap().join().unwrap();
    Ok(())
}
