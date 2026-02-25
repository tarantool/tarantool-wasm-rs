//! High level management of WebAssembly modules executed within Tarantool.
//!
//! This module contains functions exposed to the Lua layer along with runtime
//! helpers required to load, run and interact with WASM components.

mod calls;
mod exports;
mod linker;
mod threading;

use std::{
    collections::{HashMap, HashSet},
    fs::OpenOptions,
    net::IpAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc as std_mpsc, Arc, Mutex, RwLock,
    },
    thread::JoinHandle,
    time::Duration,
};

use anyhow::{anyhow, Result};
use lazy_static::lazy_static;
use rmpv::Value as RmpvValue;
use tarantool::{coio::coio_call, fiber, trigger::on_shutdown};
use tokio::{runtime::Builder, sync::mpsc};
use wasmtime::{
    component::{
        types::ComponentItem, Component, ComponentExportIndex, HasData, Linker, ResourceTable,
    },
    Engine, Store, StoreLimits, StoreLimitsBuilder,
};
use wasmtime_wasi::cli::OutputFile;
use wasmtime_wasi::p2::pipe::MemoryInputPipe;
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};
use wasmtime_wasi_tls::{WasiTlsCtx, WasiTlsCtxBuilder};

use crate::runtime::{
    executor::{Executor, TxMode},
    wasm_fiber::{WasmFiber, WasmFiberStatus},
};

use self::{
    calls::execute_call_job,
    exports::{collect_exports, export_info_from_component_func},
    linker::{build_runtime_linker, runtime_config},
    threading::spawn_tt_thread,
};

pub(crate) struct WasmRunHandle {
    join_handle: Option<JoinHandle<Result<()>>>,
    cancel_tx: mpsc::Sender<WasmFiberStatus>,
}

impl WasmRunHandle {
    fn new(join_handle: JoinHandle<Result<()>>, cancel_tx: mpsc::Sender<WasmFiberStatus>) -> Self {
        WasmRunHandle {
            join_handle: Some(join_handle),
            cancel_tx,
        }
    }

    pub(crate) fn cancel(&self) -> Result<()> {
        let _ = self.cancel_tx.blocking_send(WasmFiberStatus::Canceled);
        Ok(())
    }

    pub(crate) fn take_join_handle(&mut self) -> Result<JoinHandle<Result<()>>> {
        self.join_handle
            .take()
            .ok_or_else(|| anyhow!("join handle already taken"))
    }
}

lazy_static! {
    static ref runtime_state: RuntimeState = RuntimeState::new();
}

#[derive(Clone, Debug)]
/// Configuration options controlling how a module is executed.
pub struct WasmConfig {
    /// Inherit environment variables from the Tarantool process.
    pub inherit_env: bool,
    /// Additional environment variables to set.
    pub env: HashMap<String, String>,
    /// Inherit command line arguments of the process.
    pub inherit_args: bool,
    /// Extra arguments passed to the module.
    pub args: Vec<String>,
    /// Inherit standard input handle.
    pub inherit_stdin: bool,
    /// Inherit standard output handle.
    pub inherit_stdout: bool,
    /// Inherit standard error handle.
    pub inherit_stderr: bool,
    /// File path to use as standard input instead of inheriting.
    pub stdin: Option<String>,
    /// File path to use as standard output instead of inheriting.
    pub stdout: Option<String>,
    /// File path to use as standard error instead of inheriting.
    pub stderr: Option<String>,
    /// Allow access to the host network.
    pub inherit_network: bool,
    /// Allow DNS name resolution.
    pub allow_ip_name_lookup: bool,
    /// Allow creating TCP sockets.
    pub allow_tcp: bool,
    /// Allow creating UDP sockets.
    pub allow_udp: bool,
    /// Restrict networking to these IP addresses.
    pub allowed_ips: Vec<String>,
    /// Restrict networking to these ports.
    pub allowed_ports: Vec<u16>,
    /// Directories pre-opened for WASI with their permissions.
    pub preopened_dirs: Vec<(String, String, DirPerms, FilePerms)>,
    /// Maximum amount of linear memory a module may allocate.
    pub memory_limit: Option<usize>,
    /// Limit on the number of Wasmtime instructions (fuel) a module may consume.
    pub max_instructions: Option<u64>,
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
            allowed_ips: Vec::new(),
            allowed_ports: Vec::new(),
            preopened_dirs: Vec::new(),
            memory_limit: None,
            max_instructions: None,
        }
    }
}

/// Mutable runtime state associated with an executing module.
pub(crate) struct WasmStates {
    pub(crate) table: ResourceTable,
    pub(crate) ctx: WasiCtx,
    pub(crate) http: WasiHttpCtx,
    pub(crate) tls: WasiTlsCtx,
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

            let allow_all_ips = config.allowed_ips.is_empty();
            let allowed_ips: HashSet<IpAddr> = config
                .allowed_ips
                .iter()
                .filter_map(|ip| ip.parse::<IpAddr>().ok())
                .collect();
            let allow_all_ports = config.allowed_ports.is_empty();
            let allowed_ports: HashSet<u16> = config.allowed_ports.iter().copied().collect();
            builder.socket_addr_check(move |addr, _| {
                let ip_allowed = allow_all_ips || allowed_ips.contains(&addr.ip());
                let port_allowed = allow_all_ports || allowed_ports.contains(&addr.port());
                Box::pin(async move { ip_allowed && port_allowed })
            });
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
            tls: WasiTlsCtxBuilder::new().build(),
            yielder: None,
            executor: Executor::TxPush, // start outside the transaction
            tx_mode: TxMode::None,
        }
    }
}

/// Thread safe wrapper providing access to [`WasmStates`] along with store
/// limits.
pub(crate) struct SyncWasmStates {
    pub(crate) state: Mutex<WasmStates>,
    pub(crate) limits: StoreLimits,
}

impl SyncWasmStates {
    pub(crate) fn new(config: WasmConfig) -> Self {
        let mut builder = StoreLimitsBuilder::new();
        if let Some(limit) = config.memory_limit {
            builder = builder.memory_size(limit);
        }
        let limits = builder.build();
        SyncWasmStates {
            state: Mutex::new(WasmStates::new(config)),
            limits,
        }
    }

    pub fn begin_tx(&mut self) {
        let mut state = self.state.lock().unwrap();
        state.tx_mode = TxMode::Started;
        state.executor = Executor::Direct;
    }

    pub fn end_tx(&mut self) {
        let mut state = self.state.lock().unwrap();
        state.tx_mode = TxMode::None;
        state.executor = Executor::TxPush;
    }

    pub fn executor(&self) -> Executor {
        self.state.lock().unwrap().executor
    }

    pub(crate) async fn yield_now(&self) {
        let maybe_yielder = {
            let guard = self.state.lock().unwrap();
            guard.yielder.clone()
        };

        if let Some(handle) = maybe_yielder {
            let _ = handle.send(WasmFiberStatus::Suspended).await;
            tokio::task::yield_now().await;
        }
    }
}

impl WasiView for SyncWasmStates {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        let state = self.state.get_mut().unwrap();
        WasiCtxView {
            ctx: &mut state.ctx,
            table: &mut state.table,
        }
    }
}

impl WasiHttpView for SyncWasmStates {
    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.state.get_mut().unwrap().http
    }

    fn table(&mut self) -> &mut ResourceTable {
        &mut self.state.get_mut().unwrap().table
    }
}

impl HasData for SyncWasmStates {
    type Data<'a> = &'a mut SyncWasmStates;
}

#[derive(Clone)]
struct LoadedModule {
    component: Component,
    config: WasmConfig,
    fuel: bool,
    export_cache: Arc<RwLock<HashMap<ExportCacheKey, ComponentExportIndex>>>,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
struct ExportCacheKey {
    interface: Option<String>,
    func: String,
}

#[derive(Clone, Debug)]
struct CallJob {
    interface: Option<String>,
    func: String,
    args: Vec<RmpvValue>,
}

impl CallJob {
    fn new(interface: Option<String>, func: String, args: Vec<RmpvValue>) -> Self {
        Self {
            interface,
            func,
            args,
        }
    }

    fn cache_key(&self) -> ExportCacheKey {
        ExportCacheKey {
            interface: self.interface.clone(),
            func: self.func.clone(),
        }
    }
}

impl From<(Option<String>, String, Vec<RmpvValue>)> for CallJob {
    fn from(value: (Option<String>, String, Vec<RmpvValue>)) -> Self {
        let (interface, func, args) = value;
        Self::new(interface, func, args)
    }
}

struct RuntimeState {
    pub engine: Engine,
    pub linker: Arc<Linker<SyncWasmStates>>,
    pub fuel_engine: Engine,
    pub fuel_linker: Arc<Linker<SyncWasmStates>>,
    pub modules: RwLock<HashMap<String, LoadedModule>>,
    pub next_uid: AtomicU64,
}

impl RuntimeState {
    pub fn new() -> Self {
        let engine = Engine::new(&runtime_config(false)).unwrap();
        let fuel_engine = Engine::new(&runtime_config(true)).unwrap();

        let linker = build_runtime_linker(&engine);
        let fuel_linker = build_runtime_linker(&fuel_engine);

        let modules = RwLock::new(HashMap::new());
        let linker_ref = Arc::new(linker);
        let fuel_linker_ref = Arc::new(fuel_linker);

        let engine_ref = engine.clone();
        let fuel_engine_ref = fuel_engine.clone();

        let _ = on_shutdown(move || {
            engine_ref.increment_epoch();
            fuel_engine_ref.increment_epoch();
            fiber::sleep(Duration::new(1, 0));
        });

        RuntimeState {
            engine,
            linker: linker_ref,
            fuel_engine,
            fuel_linker: fuel_linker_ref,
            modules,
            next_uid: AtomicU64::new(0),
        }
    }
}

/// Load a WebAssembly component from disk and register it in the runtime,
/// returning a unique module identifier.
pub fn tt_wasm_load_module(path: String, config: WasmConfig) -> Result<String> {
    let use_fuel = config.max_instructions.is_some();
    let engine = if use_fuel {
        &runtime_state.fuel_engine
    } else {
        &runtime_state.engine
    };
    match Component::from_file(engine, path.clone()) {
        Ok(module) => {
            let loaded = LoadedModule {
                component: module,
                config,
                fuel: use_fuel,
                export_cache: Arc::new(RwLock::new(HashMap::new())),
            };
            let uid = format!(
                "{}#{}",
                path,
                runtime_state.next_uid.fetch_add(1, Ordering::Relaxed)
            );
            runtime_state
                .modules
                .write()
                .unwrap()
                .insert(uid.clone(), loaded);
            Ok(uid)
        }
        Err(e) => Err(e),
    }
}

/// Drop a previously loaded module, freeing associated resources.
pub fn tt_wasm_drop_module(module_uid: String) -> Result<()> {
    let mut modules = runtime_state.modules.write().unwrap();
    if modules.remove(&module_uid).is_some() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("module `{module_uid}` not found"))
    }
}

/// Run a loaded module in a background thread, returning a join handle.
pub fn tt_wasm_run_module(module_uid: String, config: Option<WasmConfig>) -> Result<WasmRunHandle> {
    let loaded = runtime_state
        .modules
        .read()
        .unwrap()
        .get(&module_uid)
        .ok_or_else(|| anyhow::anyhow!("module `{}` not found", module_uid))?
        .clone();
    let component = loaded.component;
    let mut cfg = loaded.config;
    if let Some(over) = config {
        cfg = over;
    }
    if cfg.max_instructions.is_some() && !loaded.fuel {
        return Err(anyhow::anyhow!(
            "module `{}` was loaded without fuel support",
            module_uid
        ));
    }
    let fuel = loaded.fuel;
    let (canceler_tx, canceler_rx) = std_mpsc::channel();
    let handle = spawn_tt_thread("wasm".to_string(), move || {
        let engine = if fuel {
            &runtime_state.fuel_engine
        } else {
            &runtime_state.engine
        };
        let linker = if fuel {
            runtime_state.fuel_linker.clone()
        } else {
            runtime_state.linker.clone()
        };

        let wasi_view = SyncWasmStates::new(cfg.clone());
        let mut store = Store::new(engine, wasi_view);
        store.limiter(|state| &mut state.limits);
        if fuel {
            let limit = cfg.max_instructions.unwrap_or(u64::MAX);
            let _ = store.set_fuel(limit);
        }

        store.set_epoch_deadline(1);
        store.epoch_deadline_trap();

        let fiber = WasmFiber::new(store, linker, component);
        let _ = canceler_tx.send(fiber.canceler());
        let _ = fiber.block_on();
        Ok::<(), wasmtime::Error>(())
    })?;
    let cancel_tx = canceler_rx
        .recv()
        .map_err(|_| anyhow!("failed to receive fiber cancel handle"))?;
    Ok(WasmRunHandle::new(handle, cancel_tx))
}

/// Call a single exported function of a module.
pub fn tt_wasm_call_function(
    module_uid: String,
    interface: Option<String>,
    func: String,
    args: Vec<RmpvValue>,
) -> Result<RmpvValue> {
    let loaded = runtime_state
        .modules
        .read()
        .unwrap()
        .get(&module_uid)
        .ok_or_else(|| anyhow::anyhow!("module `{}` not found", module_uid))?
        .clone();

    let component = loaded.component;
    let cfg = loaded.config;
    let fuel = loaded.fuel;
    let export_cache = loaded.export_cache;

    let handle = spawn_tt_thread("wasm".to_string(), move || {
        let engine = if fuel {
            &runtime_state.fuel_engine
        } else {
            &runtime_state.engine
        };
        let linker = if fuel {
            runtime_state.fuel_linker.clone()
        } else {
            runtime_state.linker.clone()
        };
        let wasi_view = SyncWasmStates::new(cfg.clone());
        let mut store = Store::new(engine, wasi_view);
        store.limiter(|state| &mut state.limits);
        if fuel {
            let limit = cfg.max_instructions.unwrap_or(u64::MAX);
            let _ = store.set_fuel(limit);
        }

        store.set_epoch_deadline(1);
        store.epoch_deadline_trap();

        let rt = Builder::new_current_thread()
            .enable_time()
            .enable_io()
            .build()
            .unwrap();

        rt.block_on(async move {
            let job = CallJob::new(interface, func, args);
            execute_call_job(&mut store, &linker, &component, export_cache.as_ref(), &job).await
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

/// Call multiple exported functions of a module in sequence.
pub fn tt_wasm_call_functions(
    module_uid: String,
    calls: Vec<(Option<String>, String, Vec<RmpvValue>)>,
) -> Result<Vec<RmpvValue>> {
    let loaded = runtime_state
        .modules
        .read()
        .unwrap()
        .get(&module_uid)
        .ok_or_else(|| anyhow::anyhow!("module `{}` not found", module_uid))?
        .clone();

    let component = loaded.component;
    let cfg = loaded.config;
    let fuel = loaded.fuel;
    let export_cache = loaded.export_cache;
    let jobs = calls.into_iter().map(CallJob::from).collect::<Vec<_>>();

    let handle = spawn_tt_thread("wasm".to_string(), move || {
        let engine = if fuel {
            &runtime_state.fuel_engine
        } else {
            &runtime_state.engine
        };
        let linker = if fuel {
            runtime_state.fuel_linker.clone()
        } else {
            runtime_state.linker.clone()
        };

        let rt = Builder::new_current_thread()
            .enable_time()
            .enable_io()
            .build()
            .unwrap();

        rt.block_on(async move {
            let mut results_all = Vec::new();

            for job in jobs.into_iter() {
                let wasi_view = SyncWasmStates::new(cfg.clone());
                let mut store = Store::new(engine, wasi_view);
                store.limiter(|state| &mut state.limits);
                if fuel {
                    let limit = cfg.max_instructions.unwrap_or(u64::MAX);
                    let _ = store.set_fuel(limit);
                }

                store.set_epoch_deadline(1);
                store.epoch_deadline_trap();

                results_all.push(
                    execute_call_job(&mut store, &linker, &component, export_cache.as_ref(), &job)
                        .await?,
                );
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

/// Retrieve a map of exported functions grouped by namespace.
pub fn tt_wasm_export_names(module_uid: String) -> Result<ExportMap> {
    let loaded = runtime_state
        .modules
        .read()
        .unwrap()
        .get(&module_uid)
        .ok_or_else(|| anyhow::anyhow!("module `{}` not found", module_uid))?
        .clone();

    let component = loaded.component;
    let ty = component.component_type();
    let engine = if loaded.fuel {
        &runtime_state.fuel_engine
    } else {
        &runtime_state.engine
    };

    let mut out: ExportMap = HashMap::new();

    for (name, item) in ty.exports(engine) {
        match item {
            ComponentItem::ComponentFunc(f) => {
                out.entry("".to_string())
                    .or_default()
                    .insert(name.to_string(), export_info_from_component_func(f));
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
