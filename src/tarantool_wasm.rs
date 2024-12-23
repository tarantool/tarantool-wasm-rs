use wasmtime::component::{ResourceTable, Linker};
use wasmtime_wasi::bindings::Command;
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime::{Engine, Store};
use anyhow::Result;
use lazy_static::lazy_static;
use std::thread;
use std::collections::HashMap;
use std::sync::{Mutex, Arc, RwLock};
use wasmtime::component::Component;
use wasmtime::Config;
use crate::tx::{tx_flush, tx_push, TxCtx};
use tokio::runtime::Builder;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use crate::bindings::docs::adder::tupler;
use std::thread::JoinHandle;
use nix::sys::signal::{SigSet, Signal};

lazy_static! {
    static ref runtime_state: RuntimeState = RuntimeState::new();
}

enum WasmFiberStatus {
    Suspended,
    Finished
}

struct WasmFiber {
    rt: tokio::runtime::Runtime,
    awaiter: mpsc::Receiver<WasmFiberStatus>,
}

impl WasmFiber {
    pub fn new(mut store: Store<SyncWasmStates>, linker: Arc<Linker<SyncWasmStates>>, component: Component) -> Self {
        let rt = Builder::new_current_thread().build().unwrap();
        let (yielder, awaiter) = mpsc::channel(1);
        let finalizer = yielder.clone();
        store.data().0.lock().unwrap().yielder = Some(yielder);

        rt.spawn(async move {
            let command = Command::instantiate_async(&mut store, &component, &linker);
            command.await.unwrap().wasi_cli_run().call_run(&mut store).await;
            finalizer.send(WasmFiberStatus::Finished).await;
        });
        WasmFiber{rt, awaiter}
    }

    pub fn resume(&mut self) -> WasmFiberStatus {
        self.rt.block_on(async {
            self.awaiter.recv().await.unwrap()
        })
    }
}

pub(crate) struct WasmStates {
    pub(crate) table: ResourceTable,
    pub(crate) ctx: WasiCtx,
    pub(crate) yielder: Option<mpsc::Sender<WasmFiberStatus>>,

}

impl WasmStates {
    pub fn new() -> Self {
        let table = ResourceTable::new();
        let mut builder = WasiCtxBuilder::new();
        // TODO: make env and stdio configurable
        builder.inherit_stdio().inherit_env();
        let ctx = builder.build();
        Self {table, ctx, yielder: None}
    }
}

pub(crate) struct SyncWasmStates(pub(crate) Mutex<WasmStates>);

impl SyncWasmStates {
    pub(crate) fn new() -> Self {
        SyncWasmStates(Mutex::new(WasmStates::new()))
    }

    pub(crate) async fn yield_now(&self) {
        let yielder_handle = self.0.lock().unwrap().yielder.clone().unwrap();
        yielder_handle.send(WasmFiberStatus::Suspended).await;
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
    pub linker: Arc<Linker<SyncWasmStates>>,
    pub modules: RwLock<HashMap<String, Component>>
}

impl RuntimeState {
    pub fn new() -> Self {
        let mut config = Config::new();
        config.async_support(true);
        let engine = Engine::new(&config).unwrap();
        let mut linker = Linker::<SyncWasmStates>::new(&engine);
        wasmtime_wasi::add_to_linker_async(&mut linker);
        tupler::add_to_linker(&mut linker, |state| state);
        let modules = RwLock::new(HashMap::new());
        let linker_ref = Arc::new(linker);
        RuntimeState { engine, linker: linker_ref, modules }
    }
}

pub fn tt_wasm_load_module(path: String) -> Result<String> {
    match Component::from_file(&runtime_state.engine, path.clone()) {
        Ok(module) => {
            runtime_state.modules.write().unwrap().insert(path.clone(), module);
            Ok(path)
        },
        Err(e) => Err(e),
    }
}

fn spawn_tt_thread<F, T>(name: String, f: F) -> Result<JoinHandle<T>>
where
    F: FnOnce() -> T,
    F: Send + 'static,
    T: Send + 'static,
{
    let mut newset = SigSet::all();
    newset.remove(Signal::SIGILL);
    newset.remove(Signal::SIGBUS);
    newset.remove(Signal::SIGFPE);
    newset.remove(Signal::SIGSEGV);

    let oldset = SigSet::thread_get_mask()?;

    newset.thread_block()?;
    let handle = thread::Builder::new().name(name).spawn(f);
    oldset.thread_set_mask();

    Ok(handle?)
}

fn block_on_tx(fiber: WasmFiber) -> WasmFiber {
        let (tx, rx) = oneshot::channel();
        let (ret_tx, ret_rx) = oneshot::channel();
        tx.send(fiber);
        let cb = move || {
            let mut fiber = rx.blocking_recv().unwrap();
            fiber.resume();
            ret_tx.send(fiber);
        };

        tx_push(
            Box::new(TxCtx{cb: Box::new(cb)})
        );
        tx_flush();

        ret_rx.blocking_recv().unwrap()
}

pub(crate) fn block_on(mut fiber: WasmFiber) -> Result<()>{
    loop {
        match fiber.resume() {
            WasmFiberStatus::Suspended => {
                fiber = block_on_tx(fiber);
            },
            WasmFiberStatus::Finished => {
                return Ok(());
            },
        }
    }
}

pub fn tt_wasm_run_module(module_uid: String) -> Result<JoinHandle<Result<()>>> {
    let handle = spawn_tt_thread("wasm".to_string(), move || {
        let component = runtime_state.modules.read().unwrap().get(&module_uid).unwrap().clone();
        let linker = runtime_state.linker.clone();

        let wasi_view = SyncWasmStates::new();
        let mut store = Store::new(&runtime_state.engine, wasi_view);
        let mut fiber = WasmFiber::new(store, linker, component);
        block_on(fiber);
        Ok::<(), wasmtime::Error>(())
    })?;
    Ok(handle)
}
