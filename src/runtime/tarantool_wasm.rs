use std::{
    collections::HashMap,
    fs::OpenOptions,
    sync::{Arc, Mutex, RwLock},
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::Result;
use lazy_static::lazy_static;
use nix::sys::signal::{SigSet, Signal};
use rmpv::Value as RmpvValue;
use tarantool::{coio::coio_call, fiber, trigger::on_shutdown};
use tokio::{runtime::Builder, sync::mpsc};
use wasmtime::{
    component::{
        types::{ComponentInstance, ComponentItem},
        Component, Linker, ResourceTable, Val,
    },
    AsContext, Config, Engine, Store,
};
use wasmtime_wasi::p2::{
    pipe::MemoryInputPipe, IoView, OutputFile, WasiCtx, WasiCtxBuilder, WasiView,
};
use wasmtime_wasi::{DirPerms, FilePerms};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};
use wasmtime_wasi_tls::{LinkOptions, WasiTlsCtx};

use crate::bindings::{
    docs::adder::zlib,
    tarantool::tarantool::{
        box_tuple, error, index, key_def, msgpack, say, sequence, session, ttbox, tuple_format,
        txn, types,
    },
};
use crate::runtime::{
    convert::{pretty_type, to_component_val, val_to_rmpv},
    executor::{Executor, TxMode},
    wasm_fiber::{WasmFiber, WasmFiberStatus},
};

lazy_static! {
    static ref runtime_state: RuntimeState = RuntimeState::new();
}

#[derive(Clone)]
pub struct WasmConfig {
    pub inherit_env: bool,
    pub env: HashMap<String, String>,
    pub inherit_args: bool,
    pub args: Vec<String>,
    pub inherit_stdin: bool,
    pub inherit_stdout: bool,
    pub inherit_stderr: bool,
    pub stdin: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub inherit_network: bool,
    pub allow_ip_name_lookup: bool,
    pub allow_tcp: bool,
    pub allow_udp: bool,
    pub preopened_dirs: Vec<(String, String, DirPerms, FilePerms)>,
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            inherit_env: true,
            env: HashMap::new(),
            inherit_args: false,
            args: Vec::new(),
            inherit_stdin: true,
            inherit_stdout: true,
            inherit_stderr: true,
            stdin: None,
            stdout: None,
            stderr: None,
            inherit_network: true,
            allow_ip_name_lookup: true,
            allow_tcp: true,
            allow_udp: true,
            preopened_dirs: Vec::new(),
        }
    }
}

pub(crate) struct WasmStates {
    pub(crate) table: ResourceTable,
    pub(crate) ctx: WasiCtx,
    pub(crate) http: WasiHttpCtx,
    pub(crate) yielder: Option<mpsc::Sender<WasmFiberStatus>>,
    pub(crate) executor: Executor,
    pub(crate) tx_mode: TxMode,
}

impl WasmStates {
    pub fn new(config: WasmConfig) -> Self {
        let table = ResourceTable::new();
        let mut builder = WasiCtxBuilder::new();

        if let Some(path) = &config.stdin {
            if let Ok(bytes) = std::fs::read(path) {
                builder.stdin(MemoryInputPipe::new(bytes));
            }
        } else if config.inherit_stdin {
            builder.inherit_stdin();
        }

        if let Some(path) = &config.stdout {
            if let Ok(file) = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(path)
            {
                builder.stdout(OutputFile::new(file));
            }
        } else if config.inherit_stdout {
            builder.inherit_stdout();
        }

        if let Some(path) = &config.stderr {
            if let Ok(file) = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(path)
            {
                builder.stderr(OutputFile::new(file));
            }
        } else if config.inherit_stderr {
            builder.inherit_stderr();
        }
        if config.inherit_env {
            builder.inherit_env();
        }
        for (k, v) in config.env.iter() {
            let _ = builder.env(k, v);
        }
        if config.inherit_args {
            builder.inherit_args();
        }
        if !config.args.is_empty() {
            builder.args(&config.args);
        }
        if config.inherit_network {
            builder.inherit_network();
        }
        builder
            .allow_ip_name_lookup(config.allow_ip_name_lookup)
            .allow_tcp(config.allow_tcp)
            .allow_udp(config.allow_udp);

        for (host, guest, dir_perms, file_perms) in config.preopened_dirs.iter() {
            let _ = builder.preopened_dir(host, guest, *dir_perms, *file_perms);
        }

        let ctx = builder.build();
        Self {
            table,
            ctx,
            http: WasiHttpCtx::new(),
            yielder: None,
            executor: Executor::TxPush, // start outside the transaction
            tx_mode: TxMode::None,
        }
    }
}

pub(crate) struct SyncWasmStates(pub(crate) Mutex<WasmStates>);

impl SyncWasmStates {
    pub(crate) fn new(config: WasmConfig) -> Self {
        SyncWasmStates(Mutex::new(WasmStates::new(config)))
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
        let maybe_yielder = {
            let guard = self.0.lock().unwrap();
            guard.yielder.clone()
        };

        if let Some(handle) = maybe_yielder {
            let _ = handle.send(WasmFiberStatus::Suspended).await;
            tokio::task::yield_now().await;
        }
    }
}

impl WasiView for SyncWasmStates {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.0.get_mut().unwrap().ctx
    }
}

impl IoView for SyncWasmStates {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.0.get_mut().unwrap().table
    }
}

impl WasiHttpView for SyncWasmStates {
    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.0.get_mut().unwrap().http
    }
}

#[derive(Clone)]
struct LoadedModule {
    component: Component,
    config: WasmConfig,
}

struct RuntimeState {
    pub engine: Engine,
    pub linker: Arc<Linker<SyncWasmStates>>,
    pub modules: RwLock<HashMap<String, LoadedModule>>,
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
        let _ = wasmtime_wasi::p2::add_to_linker_async(&mut linker);
        let _ = wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker);

        let mut opts = LinkOptions::default();
        opts.tls(true);
        let _ =
            wasmtime_wasi_tls::add_to_linker(&mut linker, &mut opts, |h: &mut SyncWasmStates| {
                WasiTlsCtx::new(&mut h.0.get_mut().unwrap().table)
            });

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
        let _ = box_tuple::add_to_linker(&mut linker, |state| state);

        let _ = zlib::add_to_linker(&mut linker, |state| state);

        let modules = RwLock::new(HashMap::new());
        let linker_ref = Arc::new(linker);

        let engine_ref = engine.clone();

        let _ = on_shutdown(move || {
            engine_ref.increment_epoch();
            fiber::sleep(Duration::new(1, 0));
        });

        RuntimeState {
            engine,
            linker: linker_ref,
            modules,
        }
    }
}

pub fn tt_wasm_load_module(path: String, config: WasmConfig) -> Result<String> {
    match Component::from_file(&runtime_state.engine, path.clone()) {
        Ok(module) => {
            let loaded = LoadedModule {
                component: module,
                config,
            };
            runtime_state
                .modules
                .write()
                .unwrap()
                .insert(path.clone(), loaded);
            Ok(path)
        }
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
    let _ = oldset.thread_set_mask();

    Ok(handle?)
}

pub fn tt_wasm_run_module(
    module_uid: String,
    config: Option<WasmConfig>,
) -> Result<JoinHandle<Result<()>>> {
    let handle = spawn_tt_thread("wasm".to_string(), move || {
        let loaded = runtime_state
            .modules
            .read()
            .unwrap()
            .get(&module_uid)
            .unwrap()
            .clone();
        let component = loaded.component;
        let mut cfg = loaded.config;
        if let Some(over) = config {
            cfg = over;
        }
        let linker = runtime_state.linker.clone();

        let wasi_view = SyncWasmStates::new(cfg);
        let mut store = Store::new(&runtime_state.engine, wasi_view);

        store.set_epoch_deadline(1);
        store.epoch_deadline_trap();

        let fiber = WasmFiber::new(store, linker, component);
        let _ = fiber.block_on();
        Ok::<(), wasmtime::Error>(())
    })?;
    Ok(handle)
}

pub fn tt_wasm_call_function(
    module_uid: String,
    interface: Option<String>,
    func: String,
    args: Vec<RmpvValue>,
) -> Result<RmpvValue> {
    let handle = spawn_tt_thread("wasm".to_string(), move || {
        let loaded = runtime_state
            .modules
            .read()
            .unwrap()
            .get(&module_uid)
            .ok_or_else(|| anyhow::anyhow!("module `{}` not found", module_uid))?
            .clone();

        let component = loaded.component;
        let cfg = loaded.config;

        let linker = runtime_state.linker.clone();
        let wasi_view = SyncWasmStates::new(cfg);
        let mut store = Store::new(&runtime_state.engine, wasi_view);

        store.set_epoch_deadline(1);
        store.epoch_deadline_trap();

        let rt = Builder::new_current_thread()
            .enable_time()
            .enable_io()
            .build()
            .unwrap();

        rt.block_on(async move {
            let instance = linker.instantiate_async(&mut store, &component).await?;

            let func_index = if let Some(interface_name) = interface {
                let interface_index = component
                    .get_export_index(None, &interface_name)
                    .ok_or_else(|| {
                        anyhow::anyhow!(format!("interface export `{}` not found", interface_name))
                    })?;

                component
                    .get_export_index(Some(&interface_index), &func)
                    .ok_or_else(|| {
                        anyhow::anyhow!(format!(
                            "function `{}` not found in interface `{}`",
                            func, interface_name
                        ))
                    })?
            } else {
                component.get_export_index(None, &func).ok_or_else(|| {
                    anyhow::anyhow!(format!("function `{}` not found at top level", func))
                })?
            };

            let export = instance
                .get_func(&mut store, func_index)
                .ok_or_else(|| anyhow::anyhow!(format!("function export `{func}` not found")))?;

            let params = export.params(store.as_context());
            let param_types = params.iter().map(|(_, t)| t).collect::<Vec<_>>();

            if args.len() != param_types.len() {
                return Err(anyhow::anyhow!(
                    "argument count mismatch: expected {}, got {}",
                    param_types.len(),
                    args.len()
                ));
            }

            let mut wasm_args = Vec::new();
            for (ty, val) in param_types.iter().zip(args.iter()) {
                let v = to_component_val(ty, val)?;
                wasm_args.push(v);
            }

            let result_types = export.results(store.as_context());
            let mut results_buf = vec![Val::Bool(false); result_types.len()];

            export
                .call_async(&mut store, &wasm_args, &mut results_buf)
                .await?;

            let res_val = match results_buf.as_slice() {
                [] => RmpvValue::Nil,
                [val] => val_to_rmpv(val)?,
                many => {
                    let converted = many.iter().map(val_to_rmpv).collect::<Result<Vec<_>>>()?;
                    RmpvValue::Array(converted)
                }
            };

            Ok(res_val)
        })
    })?;

    let mut result: Result<RmpvValue> = Ok(RmpvValue::Nil);

    coio_call(
        &mut |h: Box<JoinHandle<Result<RmpvValue>>>| {
            result = h
                .join()
                .unwrap_or_else(|_| Err(anyhow::anyhow!("thread panicked")));
            0
        },
        handle,
    );

    result
}

pub fn tt_wasm_call_functions(
    module_uid: String,
    calls: Vec<(Option<String>, String, Vec<RmpvValue>)>,
) -> Result<Vec<RmpvValue>> {
    let handle = spawn_tt_thread("wasm".to_string(), move || {
        let loaded = runtime_state
            .modules
            .read()
            .unwrap()
            .get(&module_uid)
            .ok_or_else(|| anyhow::anyhow!("module `{}` not found", module_uid))?
            .clone();

        let component = loaded.component;
        let cfg = loaded.config;

        let linker = runtime_state.linker.clone();

        let rt = Builder::new_current_thread()
            .enable_time()
            .enable_io()
            .build()
            .unwrap();

        rt.block_on(async move {
            let mut results_all = Vec::new();

            for (interface, func, args) in calls.into_iter() {
                let wasi_view = SyncWasmStates::new(cfg.clone());
                let mut store = Store::new(&runtime_state.engine, wasi_view);

                store.set_epoch_deadline(1);
                store.epoch_deadline_trap();

                let instance = linker.instantiate_async(&mut store, &component).await?;

                let func_index = if let Some(interface_name) = interface {
                    let interface_index = component
                        .get_export_index(None, &interface_name)
                        .ok_or_else(|| {
                            anyhow::anyhow!(format!(
                                "interface export `{}` not found",
                                interface_name
                            ))
                        })?;

                    component
                        .get_export_index(Some(&interface_index), &func)
                        .ok_or_else(|| {
                            anyhow::anyhow!(format!(
                                "function `{}` not found in interface `{}`",
                                func, interface_name
                            ))
                        })?
                } else {
                    component.get_export_index(None, &func).ok_or_else(|| {
                        anyhow::anyhow!(format!("function `{}` not found at top level", func))
                    })?
                };

                let export = instance.get_func(&mut store, func_index).ok_or_else(|| {
                    anyhow::anyhow!(format!("function export `{func}` not found"))
                })?;

                let params = export.params(store.as_context());
                let param_types = params.iter().map(|(_, t)| t).collect::<Vec<_>>();

                if args.len() != param_types.len() {
                    return Err(anyhow::anyhow!(
                        "argument count mismatch for `{}`: expected {}, got {}",
                        func,
                        param_types.len(),
                        args.len()
                    ));
                }

                let mut wasm_args = Vec::new();
                for (ty, val) in param_types.iter().zip(args.iter()) {
                    let v = to_component_val(ty, val)?;
                    wasm_args.push(v);
                }

                let result_types = export.results(store.as_context());
                let mut results_buf = vec![Val::Bool(false); result_types.len()];

                export
                    .call_async(&mut store, &wasm_args, &mut results_buf)
                    .await?;

                let res_val = match results_buf.as_slice() {
                    [] => RmpvValue::Nil,
                    [val] => val_to_rmpv(val)?,
                    many => {
                        let converted = many.iter().map(val_to_rmpv).collect::<Result<Vec<_>>>()?;
                        RmpvValue::Array(converted)
                    }
                };

                results_all.push(res_val);
            }

            Ok(results_all)
        })
    })?;

    let mut result: Result<Vec<RmpvValue>> = Ok(Vec::new());

    coio_call(
        &mut |h: Box<JoinHandle<Result<Vec<RmpvValue>>>>| {
            result = h
                .join()
                .unwrap_or_else(|_| Err(anyhow::anyhow!("thread panicked")));
            0
        },
        handle,
    );
    result
}

#[derive(Debug, serde::Serialize)]
pub struct ExportInfo {
    pub params: Vec<(String, String)>,
    pub results: Vec<String>,
}

type ExportNamespace = HashMap<String, ExportInfo>;
type ExportMap = HashMap<String, ExportNamespace>;

fn collect_exports(
    engine: &wasmtime::Engine,
    instance: &ComponentInstance,
    namespace: &str,
    out: &mut ExportMap,
) {
    for (name, item) in instance.exports(engine) {
        match item {
            ComponentItem::ComponentFunc(f) => {
                let params = f
                    .params()
                    .map(|(n, t)| (n.to_string(), pretty_type(&t)))
                    .collect();
                let results = f.results().map(|t| pretty_type(&t)).collect();

                out.entry(namespace.to_string())
                    .or_default()
                    .insert(name.to_string(), ExportInfo { params, results });
            }

            ComponentItem::ComponentInstance(subinst) => {
                let nested_namespace = format!("{namespace}/{name}");
                collect_exports(engine, &subinst, &nested_namespace, out);
            }

            _ => {}
        }
    }
}

pub fn tt_wasm_export_names(module_uid: String) -> Result<ExportMap> {
    let component = runtime_state
        .modules
        .read()
        .unwrap()
        .get(&module_uid)
        .ok_or_else(|| anyhow::anyhow!("module `{}` not found", module_uid))?
        .component
        .clone();

    let ty = component.component_type();
    let engine = &runtime_state.engine;

    let mut out: ExportMap = HashMap::new();

    for (name, item) in ty.exports(engine) {
        match item {
            ComponentItem::ComponentFunc(f) => {
                let params = f
                    .params()
                    .map(|(n, t)| (n.to_string(), pretty_type(&t)))
                    .collect();
                let results = f.results().map(|t| pretty_type(&t)).collect();

                out.entry("".to_string())
                    .or_default()
                    .insert(name.to_string(), ExportInfo { params, results });
            }

            ComponentItem::ComponentInstance(inst) => {
                if name == "exports" {
                    // Exclude the `exports` instance — it is an internal interface used for module setup
                    // and is not intended to be invoked directly from Lua.
                    continue;
                }

                if name.starts_with("wasi") {
                    // Exclude `wasi:*` instances — they provide runtime support (e.g., `wasi:cli/run`)
                    // and are not designed to be called manually from user code.
                    continue;
                }
                collect_exports(engine, &inst, name, &mut out);
            }
            _ => {}
        }
    }

    Ok(out)
}
