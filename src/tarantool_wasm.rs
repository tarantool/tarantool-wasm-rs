use wasmtime::component::{ResourceTable, Linker};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime::{Engine, Store};
use anyhow::Result;
use lazy_static::lazy_static;
use std::thread;
use std::collections::HashMap;
use std::sync::{Mutex, Arc, RwLock};
use wasmtime::component::Component;
use wasmtime::Config;
use tokio::sync::mpsc;
use crate::bindings::tarantool::tarantool::{ttbox, say, error, session, msgpack, txn, types, index, sequence, key_def, tuple_format, tuple};
use std::thread::JoinHandle;
use nix::sys::signal::{SigSet, Signal};
use crate::wasm_fiber::WasmFiberStatus;
use crate::wasm_fiber::WasmFiber;
use tarantool::trigger::on_shutdown;
use tarantool::fiber;
use std::time::Duration;
use crate::executor::{Executor, TxMode};

lazy_static! {
    static ref runtime_state: RuntimeState = RuntimeState::new();
}

pub(crate) struct WasmStates {
    pub(crate) table: ResourceTable,
    pub(crate) ctx: WasiCtx,
    pub(crate) yielder: Option<mpsc::Sender<WasmFiberStatus>>,
    pub(crate) executor: Executor,
    pub(crate) tx_mode: TxMode,
}

impl WasmStates {
    pub fn new() -> Self {
        let table = ResourceTable::new();
        let mut builder = WasiCtxBuilder::new();
        // TODO: make env and stdio configurable
        builder.inherit_stdio()
            .inherit_env()
            .inherit_network()
            .allow_ip_name_lookup(true);

        let ctx = builder.build();
        Self {
            table: table,
            ctx: ctx,
            yielder: None,
            executor: Executor::TxPush, // start outside the transaction
            tx_mode: TxMode::None,
        }
    }
}

pub(crate) struct SyncWasmStates(pub(crate) Mutex<WasmStates>);

impl SyncWasmStates {
    pub(crate) fn new() -> Self {
        SyncWasmStates(Mutex::new(WasmStates::new()))
    }

    pub fn begin_tx(&mut self) {
        self.0.lock().unwrap().tx_mode = TxMode::Started;
        self.0.lock().unwrap().executor = Executor::Direct;
    }

    pub fn end_tx(&mut self) {
        self.0.lock().unwrap().tx_mode = TxMode::None;
        self.0.lock().unwrap().executor = Executor::TxPush;
    }

    pub fn executor(&self) -> Executor {
        self.0.lock().unwrap().executor
    }

    pub(crate) async fn yield_now(&self) {
        let yielder_handle = self.0.lock().unwrap().yielder.clone().unwrap();
        let _ = yielder_handle.send(WasmFiberStatus::Suspended).await;
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

fn get_configuration() -> Config {
        let mut config = Config::new();
        /*
         * Async support is required to implement rescheduling into the TX
         * thread. In non-async mode WASM is heavily dependent on the
         * thread-local storage. In async mode it changes to the task-local
         * storage. This way we can run our module within a Tokio runtime
         * task and reschedule that task instead.
         */
        config.async_support(true);

        /*
         * We use epoch-based interruptions to implement graceful shutdown.
         * Epoch is a global tick counter managed by the engine. We can setup
         * a limiting epoch for each module, after which it will halt. With
         * that technique we can set the limit at 1 tick for all modules and
         * then halt all modules at once by incrementing the epoch.
         */
        config.epoch_interruption(true);
        config
}

impl RuntimeState {
    pub fn new() -> Self {
        let config = get_configuration();
        let engine = Engine::new(&config).unwrap();
        let mut linker = Linker::<SyncWasmStates>::new(&engine);
        let _ = wasmtime_wasi::add_to_linker_async(&mut linker);

        let _ = ttbox::add_to_linker(&mut linker, |state| state);
        let _ = say::add_to_linker(&mut linker, |state| state);
        let _ = error::add_to_linker(&mut linker, |state| state);
        let _ = session::add_to_linker(&mut linker, |state| state);
        let _ = msgpack::add_to_linker(&mut linker, |state| state);
        let _ = txn::add_to_linker(&mut linker, |state| state);
        let _ = types::add_to_linker(&mut linker, |state| state);
        let _ = index::add_to_linker(&mut linker, |state| state);
        let _ = sequence::add_to_linker(&mut linker, |state| state);
        let _ = key_def::add_to_linker(&mut linker, |state| state);
        let _ = tuple_format::add_to_linker(&mut linker, |state| state);
        let _ = tuple::add_to_linker(&mut linker, |state| state);

        // wasmtime_wasi::bindings::sockets::instance_network::Host

        let modules = RwLock::new(HashMap::new());
        let linker_ref = Arc::new(linker);

        let engine_ref = engine.clone();

        let _ = on_shutdown(move || {
            engine_ref.increment_epoch();
            fiber::sleep(Duration::new(1, 0));
        });

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
    let _ =  oldset.thread_set_mask();

    Ok(handle?)
}

pub fn tt_wasm_run_module(module_uid: String) -> Result<JoinHandle<Result<()>>> {
    let handle = spawn_tt_thread("wasm".to_string(), move || {
        let component = runtime_state.modules.read().unwrap().get(&module_uid).unwrap().clone();
        let linker = runtime_state.linker.clone();

        let wasi_view = SyncWasmStates::new();
        let mut store = Store::new(&runtime_state.engine, wasi_view);

        store.set_epoch_deadline(1);
        store.epoch_deadline_trap();

        let fiber = WasmFiber::new(store, linker, component);
        let _ = fiber.block_on();
        Ok::<(), wasmtime::Error>(())
    })?;
    Ok(handle)
}
