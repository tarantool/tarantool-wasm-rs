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
use tokio::runtime::Builder;
use tokio::sync::mpsc;
use crate::bindings::docs::adder::tupler;


lazy_static! {
    static ref runtime_state: Arc<Mutex<RuntimeState>> = Arc::new(Mutex::new(RuntimeState::new()));
}

pub(crate) struct WasmStates {
    pub(crate) table: ResourceTable,
    pub(crate) ctx: WasiCtx,
    pub(crate) yielder: Option<mpsc::Sender<()>>,

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

pub(crate) struct SyncWasmStates(pub(crate) Mutex<WasmStates>);

impl SyncWasmStates {
    pub(crate) fn new() -> Self {
        SyncWasmStates(Mutex::new(WasmStates::new()))
    }

    pub(crate) async fn yield_now(&self) {
        let yielder_handle = self.0.lock().unwrap().yielder.clone().unwrap();
        yielder_handle.send(()).await;
        tokio::task::yield_now().await;
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
        let finalizer = tx.clone();
        store.data().0.lock().unwrap().yielder = Some(tx);

        rt.block_on(async {
            rt.spawn(async move {
                let command = Command::instantiate_async(&mut store, &component, &linker);
                command.await.unwrap().wasi_cli_run().call_run(&mut store).await;
                finalizer.send(()).await;
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
