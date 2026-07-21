//! High level management of WebAssembly modules executed within Tarantool.
//!
//! This module contains functions exposed to the Lua layer along with runtime
//! helpers required to load, run and interact with WASM components.

mod calls;
mod exports;
mod linker;
mod threading;

use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::{self, OpenOptions},
    net::IpAddr,
    path::{Path, PathBuf},
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
        types::ComponentItem, Component, ComponentExportIndex, HasData, Instance, Linker,
        ResourceTable,
    },
    Engine, Store, StoreLimits, StoreLimitsBuilder,
};
use wasmtime_wasi::cli::OutputFile;
use wasmtime_wasi::p2::pipe::MemoryInputPipe;
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};
use wasmtime_wasi_http::{
    p2::{WasiHttpCtxView, WasiHttpView},
    WasiHttpCtx,
};
use wasmtime_wasi_tls::{WasiTlsCtx, WasiTlsCtxBuilder, WasiTlsCtxView, WasiTlsView};

use crate::runtime::{
    convert::{msgpack_bytes_to_rmpv, rmpv_to_msgpack_bytes},
    executor::{Executor, TxMode},
    wasm_fiber::{WasmFiber, WasmFiberStatus},
};

use self::{
    calls::{execute_call_job, execute_call_job_on_instance},
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

#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// Enable shared interpreter mode for this module.
    pub interpreter: bool,
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
            interpreter: false,
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
    fn http(&mut self) -> WasiHttpCtxView<'_> {
        let state = self.state.get_mut().unwrap();
        WasiHttpCtxView {
            ctx: &mut state.http,
            table: &mut state.table,
            hooks: Default::default(),
        }
    }
}

impl WasiTlsView for SyncWasmStates {
    fn tls(&mut self) -> WasiTlsCtxView<'_> {
        let state = self.state.get_mut().unwrap();
        WasiTlsCtxView {
            ctx: &mut state.tls,
            table: &mut state.table,
        }
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
    mode: ModuleMode,
}

#[derive(Clone)]
struct CachedComponent {
    source: Arc<[u8]>,
    component: Component,
}

const COMPONENT_CACHE_CAPACITY: usize = 32;

#[derive(Default)]
struct ComponentCache {
    entries: HashMap<(PathBuf, bool), CachedComponent>,
    insertion_order: VecDeque<(PathBuf, bool)>,
}

impl ComponentCache {
    fn insert(&mut self, key: (PathBuf, bool), component: CachedComponent) {
        if let std::collections::hash_map::Entry::Occupied(mut entry) =
            self.entries.entry(key.clone())
        {
            entry.insert(component);
            return;
        }

        if self.entries.len() == COMPONENT_CACHE_CAPACITY {
            if let Some(oldest) = self.insertion_order.pop_front() {
                self.entries.remove(&oldest);
            }
        }

        self.insertion_order.push_back(key.clone());
        self.entries.insert(key, component);
    }
}

#[derive(Clone)]
enum ModuleMode {
    Regular,
    Interpreter(Arc<SharedInterpreterEntry>),
}

struct SharedInterpreterEntry {
    path_key: String,
    component: Component,
    config: WasmConfig,
    fuel: bool,
    export_cache: Arc<RwLock<HashMap<ExportCacheKey, ComponentExportIndex>>>,
    cmd_tx: std_mpsc::Sender<InterpreterCommand>,
    join_handle: Mutex<Option<JoinHandle<Result<()>>>>,
    refcount: AtomicU64,
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

enum SessionCommand {
    Call(CallJob, std_mpsc::Sender<Result<RmpvValue>>),
    Batch(Vec<CallJob>, std_mpsc::Sender<Result<Vec<RmpvValue>>>),
    Close(std_mpsc::Sender<Result<()>>),
}

enum InterpreterCommand {
    Exec(String, std_mpsc::Sender<Result<()>>),
    Call {
        func: String,
        args_msgpack: Vec<u8>,
        result_tx: std_mpsc::Sender<Result<Vec<u8>>>,
    },
    Close(std_mpsc::Sender<Result<()>>),
}

struct WasmSession {
    module_uid: String,
    cmd_tx: std_mpsc::Sender<SessionCommand>,
    join_handle: Mutex<Option<JoinHandle<Result<()>>>>,
}

struct RuntimeState {
    pub engine: Engine,
    pub linker: Arc<Linker<SyncWasmStates>>,
    pub fuel_engine: Engine,
    pub fuel_linker: Arc<Linker<SyncWasmStates>>,
    pub components: RwLock<ComponentCache>,
    pub modules: RwLock<HashMap<String, LoadedModule>>,
    pub shared_interpreters: RwLock<HashMap<String, Arc<SharedInterpreterEntry>>>,
    pub sessions: RwLock<HashMap<String, Arc<WasmSession>>>,
    pub next_uid: AtomicU64,
    pub next_session_uid: AtomicU64,
}

impl RuntimeState {
    pub fn new() -> Self {
        let engine = Engine::new(&runtime_config(false)).unwrap();
        let fuel_engine = Engine::new(&runtime_config(true)).unwrap();

        let linker = build_runtime_linker(&engine);
        let fuel_linker = build_runtime_linker(&fuel_engine);

        let components = RwLock::new(ComponentCache::default());
        let modules = RwLock::new(HashMap::new());
        let shared_interpreters = RwLock::new(HashMap::new());
        let sessions = RwLock::new(HashMap::new());
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
            components,
            modules,
            shared_interpreters,
            sessions,
            next_uid: AtomicU64::new(0),
            next_session_uid: AtomicU64::new(0),
        }
    }
}

fn load_component(path: &Path, use_fuel: bool) -> Result<Component> {
    let source: Arc<[u8]> = fs::read(path)
        .map_err(|err| anyhow!("failed to read component `{}`: {err}", path.display()))?
        .into();
    let cache_key = (path.to_path_buf(), use_fuel);

    if let Some(cached) = runtime_state
        .components
        .read()
        .unwrap()
        .entries
        .get(&cache_key)
    {
        if cached.source == source {
            return Ok(cached.component.clone());
        }
    }

    let engine = if use_fuel {
        &runtime_state.fuel_engine
    } else {
        &runtime_state.engine
    };
    let component = Component::from_binary(engine, &source)?;

    let mut components = runtime_state.components.write().unwrap();
    if let Some(cached) = components.entries.get(&cache_key) {
        if cached.source == source {
            return Ok(cached.component.clone());
        }
    }
    components.insert(
        cache_key,
        CachedComponent {
            source,
            component: component.clone(),
        },
    );

    Ok(component)
}

/// Load a WebAssembly component from disk and register it in the runtime,
/// returning a unique module identifier.
pub fn tt_wasm_load_module(path: String, config: WasmConfig) -> Result<String> {
    let use_fuel = config.max_instructions.is_some();
    let canonical_path = PathBuf::from(&path)
        .canonicalize()
        .map_err(|err| anyhow!("failed to canonicalize `{path}`: {err}"))?;

    let loaded = if config.interpreter {
        let path_key = canonical_path.to_string_lossy().to_string();

        let entry = {
            let mut registry = runtime_state.shared_interpreters.write().unwrap();
            if let Some(existing) = registry.get(&path_key) {
                if existing.config != config {
                    return Err(anyhow!(
                        "interpreter `{path_key}` already loaded with a different config"
                    ));
                }
                existing.refcount.fetch_add(1, Ordering::Relaxed);
                Arc::clone(existing)
            } else {
                let component = load_component(&canonical_path, use_fuel)?;
                let export_cache = Arc::new(RwLock::new(HashMap::new()));
                let (cmd_tx, cmd_rx) = std_mpsc::channel();
                let (ready_tx, ready_rx) = std_mpsc::channel();

                let worker = spawn_tt_thread("wasm-interpreter".to_string(), {
                    let path_key_for_thread = path_key.clone();
                    let component_for_thread = component.clone();
                    let config_for_thread = config.clone();
                    let export_cache_for_thread = Arc::clone(&export_cache);
                    move || {
                        run_interpreter_worker(
                            path_key_for_thread,
                            component_for_thread,
                            config_for_thread,
                            use_fuel,
                            export_cache_for_thread,
                            ready_tx,
                            cmd_rx,
                        )
                    }
                })?;

                if let Err(err) = recv_result_via_coio(ready_rx) {
                    let _ = join_handle_via_coio(worker);
                    return Err(err);
                }

                let created = Arc::new(SharedInterpreterEntry {
                    path_key: path_key.clone(),
                    component: component.clone(),
                    config: config.clone(),
                    fuel: use_fuel,
                    export_cache: Arc::clone(&export_cache),
                    cmd_tx,
                    join_handle: Mutex::new(Some(worker)),
                    refcount: AtomicU64::new(1),
                });
                registry.insert(path_key.clone(), Arc::clone(&created));
                created
            }
        };

        LoadedModule {
            component: entry.component.clone(),
            config: entry.config.clone(),
            fuel: entry.fuel,
            export_cache: Arc::clone(&entry.export_cache),
            mode: ModuleMode::Interpreter(entry),
        }
    } else {
        let component = load_component(&canonical_path, use_fuel)?;
        LoadedModule {
            component,
            config,
            fuel: use_fuel,
            export_cache: Arc::new(RwLock::new(HashMap::new())),
            mode: ModuleMode::Regular,
        }
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

/// Drop a previously loaded module, freeing associated resources.
pub fn tt_wasm_drop_module(module_uid: String) -> Result<()> {
    let mode = {
        let mut modules = runtime_state.modules.write().unwrap();
        let loaded = modules
            .remove(&module_uid)
            .ok_or_else(|| anyhow::anyhow!("module `{module_uid}` not found"))?;
        loaded.mode
    };

    let mut sessions_to_close = Vec::new();
    {
        let mut sessions = runtime_state.sessions.write().unwrap();
        sessions.retain(|session_handle, session| {
            if session.module_uid == module_uid {
                sessions_to_close.push((session_handle.clone(), Arc::clone(session)));
                false
            } else {
                true
            }
        });
    }

    for (session_handle, session) in sessions_to_close {
        close_session_impl(&session_handle, session)?;
    }

    if let ModuleMode::Interpreter(entry) = mode {
        let previous = entry.refcount.fetch_sub(1, Ordering::Relaxed);
        if previous == 0 {
            entry.refcount.store(0, Ordering::Relaxed);
            return Err(anyhow!(
                "interpreter refcount underflow for path `{}`",
                entry.path_key
            ));
        }

        if previous == 1 {
            let path_key = entry.path_key.clone();
            {
                let mut registry = runtime_state.shared_interpreters.write().unwrap();
                if registry
                    .get(&entry.path_key)
                    .map(|current| Arc::ptr_eq(current, &entry))
                    .unwrap_or(false)
                {
                    registry.remove(&entry.path_key);
                }
            }
            close_interpreter_impl(&path_key, entry)?;
        }
    }

    Ok(())
}

fn recv_result_via_coio<T: Send + 'static>(receiver: std_mpsc::Receiver<Result<T>>) -> Result<T> {
    let mut result: Result<T> = Err(anyhow!("failed to receive worker response"));
    coio_call(
        &mut |rx: Box<std_mpsc::Receiver<Result<T>>>| {
            result = rx
                .recv()
                .unwrap_or_else(|_| Err(anyhow!("worker stopped unexpectedly")));
            0
        },
        receiver,
    );
    result
}

fn join_handle_via_coio<T: Send + 'static>(handle: JoinHandle<Result<T>>) -> Result<T> {
    let mut result: Result<T> = Err(anyhow!("failed to join worker thread"));
    coio_call(
        &mut |h: Box<JoinHandle<Result<T>>>| {
            result = h.join().unwrap_or_else(|_| Err(anyhow!("thread panicked")));
            0
        },
        handle,
    );
    result
}

fn init_store(engine: &Engine, config: &WasmConfig, with_fuel: bool) -> Store<SyncWasmStates> {
    let wasi_view = SyncWasmStates::new(config.clone());
    let mut store = Store::new(engine, wasi_view);
    store.limiter(|state| &mut state.limits);
    if with_fuel {
        let limit = config.max_instructions.unwrap_or(u64::MAX);
        let _ = store.set_fuel(limit);
    }
    store.set_epoch_deadline(1);
    store.epoch_deadline_trap();
    store
}

fn reset_store_fuel(store: &mut Store<SyncWasmStates>, with_fuel: bool, config: &WasmConfig) {
    if with_fuel {
        let limit = config.max_instructions.unwrap_or(u64::MAX);
        let _ = store.set_fuel(limit);
    }
}

fn decode_result_tuple(value: &RmpvValue, op: &str) -> Result<Option<RmpvValue>> {
    let values = match value.as_array() {
        Some(values) => values,
        None => return Ok(None),
    };

    if values.len() != 2 {
        return Ok(None);
    }

    let is_ok = match values[0].as_bool() {
        Some(is_ok) => is_ok,
        None => return Ok(None),
    };

    if is_ok {
        Ok(Some(values[1].clone()))
    } else if let Some(err) = values[1].as_str() {
        Err(anyhow!("interpreter `{op}` failed: {err}"))
    } else {
        Err(anyhow!(
            "interpreter `{op}` failed with non-string error: {:?}",
            values[1]
        ))
    }
}

fn decode_exec_response(value: RmpvValue) -> Result<()> {
    if decode_result_tuple(&value, "exec")?.is_some() {
        return Ok(());
    }

    match value {
        RmpvValue::Nil => Ok(()),
        RmpvValue::Boolean(true) => Ok(()),
        other => Err(anyhow!(
            "unexpected `exec` return type: expected unit or result<unit, string>, got {other:?}"
        )),
    }
}

fn decode_call_response(value: RmpvValue) -> Result<Vec<u8>> {
    let payload = match decode_result_tuple(&value, "call")? {
        Some(payload) => payload,
        None => value,
    };
    rmpv_to_u8_vec(payload, "call")
}

fn rmpv_to_u8_vec(value: RmpvValue, op: &str) -> Result<Vec<u8>> {
    let items = value
        .as_array()
        .ok_or_else(|| anyhow!("`{op}` success payload must be list<u8>"))?;

    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let byte = item
            .as_u64()
            .ok_or_else(|| anyhow!("`{op}` success payload must contain only integers"))?;
        if byte > u8::MAX as u64 {
            return Err(anyhow!("`{op}` payload byte out of range: {byte}"));
        }
        out.push(byte as u8);
    }
    Ok(out)
}

struct InterpreterJobCtx<'a> {
    store: &'a mut Store<SyncWasmStates>,
    rt: &'a tokio::runtime::Runtime,
    instance: &'a Instance,
    component: &'a Component,
    export_cache: &'a RwLock<HashMap<ExportCacheKey, ComponentExportIndex>>,
    with_fuel: bool,
    config: &'a WasmConfig,
}

fn run_interpreter_exec_job(
    ctx: InterpreterJobCtx<'_>,
    code: String,
) -> Result<()> {
    reset_store_fuel(ctx.store, ctx.with_fuel, ctx.config);
    ctx.rt.block_on(async {
        let job = CallJob::new(None, "exec".to_string(), vec![RmpvValue::from(code)]);
        execute_call_job_on_instance(
            ctx.store,
            ctx.instance,
            ctx.component,
            ctx.export_cache,
            &job,
        )
        .await
    })
    .and_then(decode_exec_response)
}

fn run_interpreter_call_job(
    ctx: InterpreterJobCtx<'_>,
    func: String,
    args_msgpack: Vec<u8>,
) -> Result<Vec<u8>> {
    let args_array = args_msgpack
        .iter()
        .map(|b| RmpvValue::from(*b as u64))
        .collect::<Vec<_>>();

    reset_store_fuel(ctx.store, ctx.with_fuel, ctx.config);
    ctx.rt.block_on(async {
        let job = CallJob::new(
            None,
            "call".to_string(),
            vec![RmpvValue::from(func), RmpvValue::Array(args_array)],
        );
        execute_call_job_on_instance(
            ctx.store,
            ctx.instance,
            ctx.component,
            ctx.export_cache,
            &job,
        )
        .await
    })
    .and_then(decode_call_response)
}

fn instantiate_interpreter_instance(
    path_key: &str,
    engine: &Engine,
    linker: &Linker<SyncWasmStates>,
    component: &Component,
    config: &WasmConfig,
    with_fuel: bool,
    rt: &tokio::runtime::Runtime,
) -> Result<(Store<SyncWasmStates>, Instance)> {
    let mut store = init_store(engine, config, with_fuel);
    let instance = rt
        .block_on(linker.instantiate_async(&mut store, component))
        .map_err(|err| anyhow!("failed to instantiate interpreter `{path_key}`: {err}"))?;
    Ok((store, instance))
}

fn run_interpreter_worker(
    path_key: String,
    component: Component,
    config: WasmConfig,
    with_fuel: bool,
    export_cache: Arc<RwLock<HashMap<ExportCacheKey, ComponentExportIndex>>>,
    ready_tx: std_mpsc::Sender<Result<()>>,
    cmd_rx: std_mpsc::Receiver<InterpreterCommand>,
) -> Result<()> {
    let engine = if with_fuel {
        &runtime_state.fuel_engine
    } else {
        &runtime_state.engine
    };
    let linker = if with_fuel {
        runtime_state.fuel_linker.clone()
    } else {
        runtime_state.linker.clone()
    };

    let rt = Builder::new_current_thread()
        .enable_time()
        .enable_io()
        .build()
        .unwrap();

    let (mut store, mut instance) = match instantiate_interpreter_instance(
        &path_key,
        engine,
        linker.as_ref(),
        &component,
        &config,
        with_fuel,
        &rt,
    ) {
        Ok(v) => v,
        Err(init_error) => {
            let _ = ready_tx.send(Err(anyhow!("{init_error}")));
            return Err(init_error);
        }
    };

    let _ = ready_tx.send(Ok(()));

    while let Ok(command) = cmd_rx.recv() {
        match command {
            InterpreterCommand::Exec(code, result_tx) => {
                let result = run_interpreter_exec_job(
                    InterpreterJobCtx {
                        store: &mut store,
                        rt: &rt,
                        instance: &instance,
                        component: &component,
                        export_cache: export_cache.as_ref(),
                        with_fuel,
                        config: &config,
                    },
                    code,
                );
                match result {
                    Ok(()) => {
                        let _ = result_tx.send(Ok(()));
                    }
                    Err(err) => {
                        match instantiate_interpreter_instance(
                            &path_key,
                            engine,
                            linker.as_ref(),
                            &component,
                            &config,
                            with_fuel,
                            &rt,
                        ) {
                            Ok((new_store, new_instance)) => {
                                store = new_store;
                                instance = new_instance;
                                let _ = result_tx.send(Err(err));
                            }
                            Err(recover_err) => {
                                let combined = anyhow!(
                                    "{err}; interpreter `{path_key}` entered unrecoverable state: {recover_err}"
                                );
                                let _ = result_tx.send(Err(combined));
                                break;
                            }
                        }
                    }
                }
            }
            InterpreterCommand::Call {
                func,
                args_msgpack,
                result_tx,
            } => {
                let result = run_interpreter_call_job(
                    InterpreterJobCtx {
                        store: &mut store,
                        rt: &rt,
                        instance: &instance,
                        component: &component,
                        export_cache: export_cache.as_ref(),
                        with_fuel,
                        config: &config,
                    },
                    func,
                    args_msgpack,
                );
                match result {
                    Ok(raw) => {
                        let _ = result_tx.send(Ok(raw));
                    }
                    Err(err) => {
                        match instantiate_interpreter_instance(
                            &path_key,
                            engine,
                            linker.as_ref(),
                            &component,
                            &config,
                            with_fuel,
                            &rt,
                        ) {
                            Ok((new_store, new_instance)) => {
                                store = new_store;
                                instance = new_instance;
                                let _ = result_tx.send(Err(err));
                            }
                            Err(recover_err) => {
                                let combined = anyhow!(
                                    "{err}; interpreter `{path_key}` entered unrecoverable state: {recover_err}"
                                );
                                let _ = result_tx.send(Err(combined));
                                break;
                            }
                        }
                    }
                }
            }
            InterpreterCommand::Close(done_tx) => {
                let _ = done_tx.send(Ok(()));
                break;
            }
        }
    }

    Ok(())
}

fn run_session_worker(
    module_uid: String,
    component: Component,
    config: WasmConfig,
    with_fuel: bool,
    export_cache: Arc<RwLock<HashMap<ExportCacheKey, ComponentExportIndex>>>,
    ready_tx: std_mpsc::Sender<Result<()>>,
    cmd_rx: std_mpsc::Receiver<SessionCommand>,
) -> Result<()> {
    let engine = if with_fuel {
        &runtime_state.fuel_engine
    } else {
        &runtime_state.engine
    };
    let linker = if with_fuel {
        runtime_state.fuel_linker.clone()
    } else {
        runtime_state.linker.clone()
    };

    let mut store = init_store(engine, &config, with_fuel);

    let rt = Builder::new_current_thread()
        .enable_time()
        .enable_io()
        .build()
        .unwrap();

    let instance = match rt.block_on(linker.instantiate_async(&mut store, &component)) {
        Ok(instance) => instance,
        Err(err) => {
            let init_error =
                anyhow!("failed to instantiate session for module `{module_uid}`: {err}");
            let _ = ready_tx.send(Err(anyhow!("{init_error}")));
            return Err(init_error);
        }
    };

    let _ = ready_tx.send(Ok(()));

    while let Ok(command) = cmd_rx.recv() {
        match command {
            SessionCommand::Call(job, result_tx) => {
                reset_store_fuel(&mut store, with_fuel, &config);
                let result = rt.block_on(async {
                    execute_call_job_on_instance(
                        &mut store,
                        &instance,
                        &component,
                        export_cache.as_ref(),
                        &job,
                    )
                    .await
                });
                let _ = result_tx.send(result);
            }
            SessionCommand::Batch(jobs, result_tx) => {
                let mut results_all = Vec::with_capacity(jobs.len());
                let mut batch_result = Ok(());
                for job in jobs {
                    reset_store_fuel(&mut store, with_fuel, &config);
                    match rt.block_on(async {
                        execute_call_job_on_instance(
                            &mut store,
                            &instance,
                            &component,
                            export_cache.as_ref(),
                            &job,
                        )
                        .await
                    }) {
                        Ok(value) => results_all.push(value),
                        Err(err) => {
                            batch_result = Err(err);
                            break;
                        }
                    }
                }

                let _ = result_tx.send(match batch_result {
                    Ok(()) => Ok(results_all),
                    Err(err) => Err(err),
                });
            }
            SessionCommand::Close(done_tx) => {
                let _ = done_tx.send(Ok(()));
                break;
            }
        }
    }

    Ok(())
}

fn close_session_impl(session_handle: &str, session: Arc<WasmSession>) -> Result<()> {
    let (close_tx, close_rx) = std_mpsc::channel();
    if session.cmd_tx.send(SessionCommand::Close(close_tx)).is_ok() {
        recv_result_via_coio(close_rx)?;
    }

    if let Some(handle) = session.join_handle.lock().unwrap().take() {
        join_handle_via_coio(handle)
            .map_err(|err| anyhow!("session `{session_handle}` worker failed: {err}"))?;
    }
    Ok(())
}

fn close_interpreter_impl(path_key: &str, entry: Arc<SharedInterpreterEntry>) -> Result<()> {
    let (close_tx, close_rx) = std_mpsc::channel();
    if entry
        .cmd_tx
        .send(InterpreterCommand::Close(close_tx))
        .is_ok()
    {
        recv_result_via_coio(close_rx)?;
    }

    if let Some(handle) = entry.join_handle.lock().unwrap().take() {
        join_handle_via_coio(handle)
            .map_err(|err| anyhow!("interpreter `{path_key}` worker failed: {err}"))?;
    }

    Ok(())
}

/// Open a long-lived WASM session with a persistent component instance.
pub fn tt_wasm_open_session(module_uid: String, config: Option<WasmConfig>) -> Result<String> {
    let loaded = runtime_state
        .modules
        .read()
        .unwrap()
        .get(&module_uid)
        .ok_or_else(|| anyhow::anyhow!("module `{}` not found", module_uid))?
        .clone();

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

    let component = loaded.component;
    let fuel = loaded.fuel;
    let export_cache = loaded.export_cache;

    let (cmd_tx, cmd_rx) = std_mpsc::channel();
    let (ready_tx, ready_rx) = std_mpsc::channel();

    let module_uid_for_thread = module_uid.clone();
    let worker = spawn_tt_thread("wasm-session".to_string(), move || {
        run_session_worker(
            module_uid_for_thread,
            component,
            cfg,
            fuel,
            export_cache,
            ready_tx,
            cmd_rx,
        )
    })?;

    if let Err(err) = recv_result_via_coio(ready_rx) {
        let _ = join_handle_via_coio(worker);
        return Err(err);
    }

    let session_handle = format!(
        "{}::session#{}",
        module_uid,
        runtime_state
            .next_session_uid
            .fetch_add(1, Ordering::Relaxed)
    );

    runtime_state.sessions.write().unwrap().insert(
        session_handle.clone(),
        Arc::new(WasmSession {
            module_uid,
            cmd_tx,
            join_handle: Mutex::new(Some(worker)),
        }),
    );

    Ok(session_handle)
}

/// Call a single export through an opened long-lived session.
pub fn tt_wasm_session_call_function(
    session_handle: String,
    interface: Option<String>,
    func: String,
    args: Vec<RmpvValue>,
) -> Result<RmpvValue> {
    let session = runtime_state
        .sessions
        .read()
        .unwrap()
        .get(&session_handle)
        .cloned()
        .ok_or_else(|| anyhow!("session `{session_handle}` not found"))?;

    let (result_tx, result_rx) = std_mpsc::channel();
    session
        .cmd_tx
        .send(SessionCommand::Call(
            CallJob::new(interface, func, args),
            result_tx,
        ))
        .map_err(|_| anyhow!("session `{session_handle}` is closed"))?;

    recv_result_via_coio(result_rx)
}

/// Call multiple exports through an opened long-lived session.
pub fn tt_wasm_session_call_functions(
    session_handle: String,
    calls: Vec<(Option<String>, String, Vec<RmpvValue>)>,
) -> Result<Vec<RmpvValue>> {
    let session = runtime_state
        .sessions
        .read()
        .unwrap()
        .get(&session_handle)
        .cloned()
        .ok_or_else(|| anyhow!("session `{session_handle}` not found"))?;

    let jobs = calls.into_iter().map(CallJob::from).collect::<Vec<_>>();
    let (result_tx, result_rx) = std_mpsc::channel();
    session
        .cmd_tx
        .send(SessionCommand::Batch(jobs, result_tx))
        .map_err(|_| anyhow!("session `{session_handle}` is closed"))?;

    recv_result_via_coio(result_rx)
}

/// Close a long-lived session and release its worker thread and instance.
pub fn tt_wasm_close_session(session_handle: String) -> Result<()> {
    let session = runtime_state
        .sessions
        .write()
        .unwrap()
        .remove(&session_handle)
        .ok_or_else(|| anyhow!("session `{session_handle}` not found"))?;

    close_session_impl(&session_handle, session)
}

fn interpreter_entry_for_module(module_uid: &str) -> Result<Arc<SharedInterpreterEntry>> {
    let loaded = runtime_state
        .modules
        .read()
        .unwrap()
        .get(module_uid)
        .cloned()
        .ok_or_else(|| anyhow!("module `{module_uid}` not found"))?;

    match loaded.mode {
        ModuleMode::Interpreter(entry) => Ok(entry),
        ModuleMode::Regular => Err(anyhow!(
            "module `{module_uid}` is not loaded in interpreter mode"
        )),
    }
}

/// Execute source code in a shared interpreter instance.
pub fn tt_wasm_interpreter_exec(module_uid: String, code: String) -> Result<()> {
    let entry = interpreter_entry_for_module(&module_uid)?;
    let (result_tx, result_rx) = std_mpsc::channel();
    entry
        .cmd_tx
        .send(InterpreterCommand::Exec(code, result_tx))
        .map_err(|_| anyhow!("interpreter for module `{module_uid}` is closed"))?;

    recv_result_via_coio(result_rx)
}

/// Call a function in a shared interpreter instance.
pub fn tt_wasm_interpreter_call(
    module_uid: String,
    func: String,
    args: Vec<RmpvValue>,
) -> Result<RmpvValue> {
    let entry = interpreter_entry_for_module(&module_uid)?;
    let args_msgpack = rmpv_to_msgpack_bytes(&RmpvValue::Array(args))?;
    let (result_tx, result_rx) = std_mpsc::channel();

    entry
        .cmd_tx
        .send(InterpreterCommand::Call {
            func,
            args_msgpack,
            result_tx,
        })
        .map_err(|_| anyhow!("interpreter for module `{module_uid}` is closed"))?;

    let raw = recv_result_via_coio(result_rx)?;
    msgpack_bytes_to_rmpv(&raw)
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
        Ok::<(), anyhow::Error>(())
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
        match item.ty {
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
