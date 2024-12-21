use wasmtime::component::{ResourceTable};
use wasmtime_wasi::bindings::Command;
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime::{Engine, Store};
use anyhow::Result;
use lazy_static::lazy_static;
use std::thread;
use std::collections::HashMap;
use std::sync::{Mutex, Arc};
use wasmtime::component::Component;
use wasmtime::Config;
use crate::tx::{tx_flush, tx_push, TxCtx};
use wasmtime::component::bindgen;
use crate::tarantool_wasm::docs::adder::tupler;
use tokio::runtime::Builder;
use tokio::sync::mpsc;
use wasmtime_wasi::async_trait;

lazy_static! {
    static ref runtime_state: Arc<Mutex<RuntimeState>> = Arc::new(Mutex::new(RuntimeState::new()));
}

pub struct WasmStates {
    table: ResourceTable,
    ctx: WasiCtx,
    yielder: Option<mpsc::Sender<()>>,

}

struct SyncWasmStates(Mutex<WasmStates>);

bindgen!({
    path: "adder.wit",
    world: "adder",
    async: true,
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
        Self { table, ctx, yielder: None.into() }
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

#[async_trait]
impl tupler::Host for SyncWasmStates {
    async fn get_tuple(&mut self, name: String, value: Vec<u8>) -> tupler::TtTuple {
        let current_thread = thread::current();
        println!("Current thread name: {:?}", current_thread.name());
        let yielder_handle = self.0.lock().unwrap().yielder.clone().unwrap();
        yielder_handle.send(()).await;
        tokio::task::yield_now().await;
        let current_thread = thread::current();
        println!("Current thread name: {:?}", current_thread.name());

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
        let yielder_handle = self.0.lock().unwrap().yielder.clone().unwrap();
        yielder_handle.send(()).await;
        tokio::task::yield_now().await;
        let current_thread = thread::current();
        println!("Current thread name: {:?}", current_thread.name());
        Ok(tup)
    }

    async fn drop(&mut self, self_: wasmtime::component::Resource<TtSpace>) -> Result<(), wasmtime::Error> {
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
    drop(state);
    let wasi_view = SyncWasmStates::new();
    let mut store = Store::new(&engine, wasi_view);

    let handle = thread::Builder::new().name("MyThread".to_string()).spawn(move || {
        let rt = Builder::new_current_thread().build().unwrap();
        let (tx, rx) = mpsc::channel(1);
        let rx_cell = Arc::new(Mutex::new(rx));
        rt.block_on(async {
            let final_tx = tx.clone();
            store.data().0.lock().unwrap().yielder = Some(tx);
            rt.spawn(async move {
                let command = Command::instantiate_async(&mut store, &component, &linker);
                command.await.unwrap().wasi_cli_run().call_run(&mut store).await;
                final_tx.send(()).await;
            });
            rx_cell.lock().unwrap().recv().await.unwrap();
        });

        let (tx, rx) = std::sync::mpsc::channel();
        let (ret_tx, ret_rx) = std::sync::mpsc::channel();
        tx.send(rt);
        let rx_cell_cp = rx_cell.clone();
        let cb = move || -> () {
            let rt = rx.recv().unwrap();
            rt.block_on(async {
                rx_cell.lock().unwrap().recv().await.unwrap();
            });
            ret_tx.send(rt);
        };
        tx_push(
            Box::new(TxCtx{cb: Box::new(cb)})
        );
        tx_flush();
        let rt = ret_rx.recv().unwrap();
        rt.block_on(async {
            rx_cell_cp.lock().unwrap().recv().await.unwrap();
        });
        Ok::<(), wasmtime::Error>(())
    });
    Ok(())
}
