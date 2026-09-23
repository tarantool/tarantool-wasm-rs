use super::*;
use crate::bindings::handles::NativeHandles;
use crate::runtime::tx_fiber::TxFiber;

use std::sync::Arc;

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
    pub preopened_dirs: Vec<(String, String, FsPerms)>,
    /// Maximum amount of linear memory a module may allocate.
    pub memory_limit: Option<usize>,
    /// Limit on the number of Wasmtime instructions (fuel) a module may consume.
    pub max_instructions: Option<u64>,
    /// Enable shared interpreter mode for this module.
    pub interpreter: bool,
    /// Load a trusted component precompiled by this runtime.
    pub precompiled: bool,
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
            precompiled: false,
        }
    }
}

/// Mutable runtime state associated with an executing module.
pub(crate) struct WasmStates {
    pub(crate) table: ResourceTable,
    pub(crate) handles: NativeHandles,
    pub(crate) ctx: WasiCtx,
    pub(crate) http: WasiHttpCtx,
    pub(crate) tls: WasiTlsCtx,
    pub(crate) yielder: Option<mpsc::Sender<WasmFiberStatus>>,
    pub(crate) executor: Executor,
    pub(crate) tx_fiber: Option<Arc<TxFiber>>,
    pub(crate) tx_mode: TxMode,
    pub(crate) caller_session_id: Option<u64>,
    pub(crate) instance_thread_id: ThreadId,
    pub(crate) call_chain: CallChain,
}

impl WasmStates {
    pub fn new(config: WasmConfig) -> Result<Self> {
        Self::new_inner(config, None)
    }

    pub(crate) fn new_with_pipe(
        config: WasmConfig,
        streams: super::pipe::PipeWasiStreams,
    ) -> Result<Self> {
        Self::new_inner(config, Some(streams))
    }

    fn new_inner(
        config: WasmConfig,
        streams: Option<super::pipe::PipeWasiStreams>,
    ) -> Result<Self> {
        let table = ResourceTable::new();
        let mut builder = WasiCtxBuilder::new();
        let has_pipe_streams = streams.is_some();

        if let Some(streams) = streams {
            streams.install(&mut builder);
        } else if let Some(path) = &config.stdin {
            let bytes = std::fs::read(path)
                .with_context(|| format!("failed to read stdin from `{path}`"))?;
            builder.stdin(MemoryInputPipe::new(bytes));
        } else if config.inherit_stdin {
            builder.inherit_stdin();
        }

        if !has_pipe_streams {
            if let Some(path) = &config.stdout {
                let file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(path)
                    .with_context(|| format!("failed to open stdout file `{path}`"))?;
                builder.stdout(OutputFile::new(file));
            } else if config.inherit_stdout {
                builder.inherit_stdout();
            }

            if let Some(path) = &config.stderr {
                let file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(path)
                    .with_context(|| format!("failed to open stderr file `{path}`"))?;
                builder.stderr(OutputFile::new(file));
            } else if config.inherit_stderr {
                builder.inherit_stderr();
            }
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
                .map(|ip| {
                    ip.parse::<IpAddr>()
                        .with_context(|| format!("invalid allowed IP address `{ip}`"))
                })
                .collect::<Result<_>>()?;
            let allow_all_ports = config.allowed_ports.is_empty();
            let allowed_ports: HashSet<u16> = config.allowed_ports.iter().copied().collect();
            builder.socket_addr_check(move |addr, reason| {
                let implicit_bind = addr.ip().is_unspecified()
                    && addr.port() == 0
                    && matches!(reason, SocketAddrUse::TcpBind | SocketAddrUse::UdpBind);
                let ip_allowed = allow_all_ips || allowed_ips.contains(&addr.ip());
                let port_allowed = allow_all_ports || allowed_ports.contains(&addr.port());
                Box::pin(async move { implicit_bind || ip_allowed && port_allowed })
            });
        }
        builder
            .allow_ip_name_lookup(config.allow_ip_name_lookup)
            .allow_tcp(config.allow_tcp)
            .allow_udp(config.allow_udp);

        for (host, guest, perms) in config.preopened_dirs.iter() {
            builder.preopened_dir(host, guest, *perms).map_err(|err| {
                anyhow!("failed to preopen directory `{host}` as `{guest}`: {err}")
            })?;
        }

        let ctx = builder.build();
        Ok(Self {
            table,
            handles: NativeHandles::new(),
            ctx,
            http: WasiHttpCtx::new(),
            tls: WasiTlsCtxBuilder::new().build(),
            yielder: None,
            executor: Executor::TxPush, // start outside the transaction
            tx_fiber: None,
            tx_mode: TxMode::None,
            caller_session_id: None,
            instance_thread_id: thread::current().id(),
            call_chain: Vec::new(),
        })
    }
}

/// Thread safe wrapper providing access to [`WasmStates`] along with store
/// limits.
pub(crate) struct SyncWasmStates {
    pub(crate) state: Mutex<WasmStates>,
    pub(crate) limits: TrackingStoreLimits,
}

pub(crate) struct TrackingStoreLimits {
    inner: StoreLimits,
    linear_memory_bytes: usize,
    pending_growth: usize,
}

impl TrackingStoreLimits {
    fn new(inner: StoreLimits) -> Self {
        Self {
            inner,
            linear_memory_bytes: 0,
            pending_growth: 0,
        }
    }
}

impl Drop for TrackingStoreLimits {
    fn drop(&mut self) {
        metrics::remove_memory_bytes(self.linear_memory_bytes);
    }
}

impl ResourceLimiter for TrackingStoreLimits {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> std::result::Result<bool, wasmtime::Error> {
        self.pending_growth = 0;
        let allowed = self.inner.memory_growing(current, desired, maximum)?;
        if allowed {
            let growth = desired.saturating_sub(current);
            self.linear_memory_bytes = self.linear_memory_bytes.saturating_add(growth);
            self.pending_growth = growth;
            metrics::add_memory_bytes(growth);
        }
        Ok(allowed)
    }

    fn memory_grow_failed(
        &mut self,
        error: wasmtime::Error,
    ) -> std::result::Result<(), wasmtime::Error> {
        let growth = std::mem::take(&mut self.pending_growth);
        self.linear_memory_bytes = self.linear_memory_bytes.saturating_sub(growth);
        metrics::remove_memory_bytes(growth);
        self.inner.memory_grow_failed(error)
    }

    fn table_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> std::result::Result<bool, wasmtime::Error> {
        self.inner.table_growing(current, desired, maximum)
    }

    fn table_grow_failed(
        &mut self,
        error: wasmtime::Error,
    ) -> std::result::Result<(), wasmtime::Error> {
        self.inner.table_grow_failed(error)
    }

    fn instances(&self) -> usize {
        self.inner.instances()
    }

    fn tables(&self) -> usize {
        self.inner.tables()
    }

    fn memories(&self) -> usize {
        self.inner.memories()
    }
}

impl SyncWasmStates {
    pub(crate) fn new(config: WasmConfig) -> Result<Self> {
        Self::new_inner(config, None)
    }

    pub(crate) fn new_with_pipe(
        config: WasmConfig,
        streams: super::pipe::PipeWasiStreams,
    ) -> Result<Self> {
        Self::new_inner(config, Some(streams))
    }

    fn new_inner(
        config: WasmConfig,
        streams: Option<super::pipe::PipeWasiStreams>,
    ) -> Result<Self> {
        let mut builder = StoreLimitsBuilder::new();
        if let Some(limit) = config.memory_limit {
            builder = builder.memory_size(limit);
        }
        let limits = TrackingStoreLimits::new(builder.build());
        Ok(SyncWasmStates {
            state: Mutex::new(match streams {
                Some(streams) => WasmStates::new_with_pipe(config, streams)?,
                None => WasmStates::new(config)?,
            }),
            limits,
        })
    }

    /// Open a transaction by spawning its dedicated fiber.
    ///
    /// All host calls made while the transaction is open are dispatched to this
    /// fiber (see [`Executor::TxFiber`]), so the fiber-local `box` transaction
    /// is preserved without keeping the guest on the TX thread.
    ///
    /// Must be called from outside the TX thread: spawning the fiber pushes work
    /// onto TX. Returns an error if that submission fails.
    pub fn begin_tx(&mut self) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        // Reuse an existing fiber: a second `begin` without an intervening
        // commit/rollback must reach the same fiber so Tarantool reports the
        // duplicate begin, rather than silently spawning a new transaction and
        // orphaning the first.
        if state.tx_fiber.is_none() {
            // `TxFiber::spawn` submits to TX; safe from the guest's OS thread.
            // The spawn callback never touches `WasmStates`, so holding the
            // lock here cannot deadlock it.
            state.tx_fiber = Some(TxFiber::spawn()?);
        }
        let fiber = Arc::clone(state.tx_fiber.as_ref().unwrap());
        state.tx_mode = TxMode::Started;
        state.executor = Executor::TxFiber(fiber);
        Ok(())
    }

    pub fn end_tx(&mut self) {
        let mut state = self.state.lock().unwrap();
        state.tx_mode = TxMode::None;
        state.executor = Executor::TxPush;
        // Dropping the handle closes the job channel; the fiber rolls back any
        // still-open transaction and exits.
        state.tx_fiber = None;
    }

    pub fn executor(&self) -> Executor {
        self.state.lock().unwrap().executor.clone()
    }
    pub(crate) fn caller_session_id(&self) -> Option<u64> {
        self.state.lock().unwrap().caller_session_id
    }
    pub(crate) fn set_caller_session_id(&mut self, session_id: Option<u64>) {
        self.state.get_mut().unwrap().caller_session_id = session_id;
    }

    pub(crate) fn enter_call(&mut self, mut call_chain: CallChain) -> Result<()> {
        let state = self.state.get_mut().unwrap();
        if call_chain.contains(&state.instance_thread_id) {
            return Err(anyhow!(
                "reentrant WASM call reached an already active component instance"
            ));
        }
        call_chain.push(state.instance_thread_id);
        state.call_chain = call_chain;
        Ok(())
    }

    pub(crate) fn call_chain(&self) -> CallChain {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .call_chain
            .clone()
    }

    // Retained for the (now unused) task-rescheduling path: transactions used
    // to migrate the guest onto TX via `yield_now`. The dedicated-fiber model
    // in `crate::runtime::tx_fiber` no longer needs it.
    #[allow(dead_code)]
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

    pub(crate) fn can_reschedule_to_tx(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .yielder
            .is_some()
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

pub(super) fn init_store(
    engine: &Engine,
    config: &WasmConfig,
    with_fuel: bool,
) -> Result<Store<SyncWasmStates>> {
    let started = std::time::Instant::now();
    let result = (|| {
        let wasi_view = SyncWasmStates::new(config.clone())?;
        let mut store = Store::new(engine, wasi_view);
        store.limiter(|state| &mut state.limits);
        if with_fuel {
            let limit = config.max_instructions.unwrap_or(u64::MAX);
            let _ = store.set_fuel(limit);
        }
        crate::runtime::wasm_fiber::configure_epoch_deadline(&mut store, None);
        Ok(store)
    })();
    metrics::observe_store_setup(started.elapsed());
    result
}

pub(super) fn reset_store_fuel(
    store: &mut Store<SyncWasmStates>,
    with_fuel: bool,
    config: &WasmConfig,
) {
    if with_fuel {
        let limit = config.max_instructions.unwrap_or(u64::MAX);
        let _ = store.set_fuel(limit);
    }
}
