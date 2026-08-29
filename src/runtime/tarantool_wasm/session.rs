use super::*;

pub(super) enum SessionCommand {
    Call(CallJob, WorkerResultSender<SessionCallResult>),
    Batch(Vec<CallJob>, WorkerResultSender<Vec<SessionCallResult>>),
    DropResource(u64, Option<WorkerResultSender<()>>),
    Close(WorkerResultSender<()>),
}

pub(super) struct WasmSession {
    pub(super) module_uid: String,
    pub(super) cmd_tx: std_mpsc::Sender<SessionCommand>,
    pub(super) join_handle: Mutex<Option<JoinHandle<Result<()>>>>,
}

type DetachedSessionWorker = (String, JoinHandle<Result<()>>);

static DETACHED_SESSION_WORKERS: Mutex<Vec<DetachedSessionWorker>> = Mutex::new(Vec::new());

pub(super) fn take_detached_session_workers() -> Vec<DetachedSessionWorker> {
    std::mem::take(&mut *DETACHED_SESSION_WORKERS.lock().unwrap())
}

impl WasmSession {
    fn worker_thread_id(&self) -> Result<ThreadId> {
        self.join_handle
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .as_ref()
            .map(|handle| handle.thread().id())
            .ok_or_else(|| anyhow!("WASM session worker is no longer available"))
    }
}

#[derive(Clone)]
pub(crate) enum WasmInstanceTarget {
    Session(String),
    Interpreter(Arc<SharedInterpreterEntry>),
}

impl WasmInstanceTarget {
    pub(crate) fn same_instance(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Session(left), Self::Session(right)) => left == right,
            (Self::Interpreter(left), Self::Interpreter(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }

    fn worker_thread_id(&self) -> Result<ThreadId> {
        match self {
            Self::Session(session_handle) => runtime_state
                .sessions
                .read()
                .unwrap_or_else(|poison| poison.into_inner())
                .get(session_handle)
                .cloned()
                .ok_or_else(|| anyhow!("session `{session_handle}` not found"))?
                .worker_thread_id(),
            Self::Interpreter(entry) => entry.worker_thread_id(),
        }
    }

    fn ensure_not_reentrant(&self) -> Result<CallChain> {
        let description = match self {
            Self::Session(session_handle) => format!("session `{session_handle}`"),
            Self::Interpreter(entry) => format!("interpreter `{}`", entry.path_key),
        };
        ensure_target_not_active(self.worker_thread_id()?, &description)
    }
}

pub(super) fn run_session_worker(
    module_uid: String,
    prepared: PreparedComponent,
    config: WasmConfig,
    with_fuel: bool,
    export_cache: Arc<ExportCache>,
    ready_tx: WorkerResultSender<()>,
    cmd_rx: std_mpsc::Receiver<SessionCommand>,
) -> Result<()> {
    let PreparedComponent {
        component,
        instance_pre,
    } = prepared;
    let engine = if with_fuel {
        &runtime_state.fuel_engine
    } else {
        &runtime_state.engine
    };
    let mut store = match init_store(engine, &config, with_fuel) {
        Ok(store) => store,
        Err(err) => {
            let init_error =
                anyhow!("failed to initialize session for module `{module_uid}`: {err:#}");
            let _ = ready_tx.send(Err(anyhow!("{init_error:#}")));
            return Err(init_error);
        }
    };

    let runtime_started = std::time::Instant::now();
    let rt = Builder::new_current_thread()
        .enable_time()
        .enable_io()
        .build()
        .unwrap();
    metrics::observe_runtime_setup(runtime_started.elapsed());

    let instantiation_started = std::time::Instant::now();
    let instance = match rt.block_on(instance_pre.instantiate_async(&mut store)) {
        Ok(instance) => instance,
        Err(err) => {
            let init_error =
                anyhow!("failed to instantiate session for module `{module_uid}`: {err}");
            let _ = ready_tx.send(Err(anyhow!("{init_error}")));
            return Err(init_error);
        }
    };
    metrics::observe_instantiation(instantiation_started.elapsed());
    let mut resources = SessionResources::new(engine, &component, &instance, &mut store);

    let _ = ready_tx.send(Ok(()));

    loop {
        let command = match cmd_rx.recv() {
            Ok(command) => command,
            Err(_) => break,
        };
        match command {
            SessionCommand::Call(job, result_tx) => {
                reset_store_fuel(&mut store, with_fuel, &config);
                let result = rt.block_on(async {
                    execute_session_call_job_on_instance(
                        &mut store,
                        &instance,
                        &component,
                        export_cache.as_ref(),
                        &mut resources,
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
                        execute_session_call_job_on_instance(
                            &mut store,
                            &instance,
                            &component,
                            export_cache.as_ref(),
                            &mut resources,
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
            SessionCommand::DropResource(id, result_tx) => {
                let result = rt.block_on(resources.drop_resource(&mut store, id));
                if let Some(result_tx) = result_tx {
                    let _ = result_tx.send(result);
                }
            }
            SessionCommand::Close(done_tx) => {
                let result = rt.block_on(resources.drop_all(&mut store));
                let _ = done_tx.send(result);
                break;
            }
        }
    }

    Ok(())
}

pub(super) fn close_session_impl(session_handle: &str, session: Arc<WasmSession>) -> Result<()> {
    let (close_tx, close_rx) = worker_result_channel();
    if session.cmd_tx.send(SessionCommand::Close(close_tx)).is_ok() {
        close_rx.recv()?;
    }

    if let Some(handle) = session.join_handle.lock().unwrap().take() {
        join_handle_via_coio(handle)
            .map_err(|err| anyhow!("session `{session_handle}` worker failed: {err}"))?;
    }
    Ok(())
}

pub fn tt_wasm_open_session(module_uid: String, config: Option<WasmConfig>) -> Result<String> {
    lifecycle::ensure_running()?;
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
    let instance_pre = loaded.instance_pre;
    let fuel = loaded.fuel;
    let export_cache = loaded.export_cache;

    let (cmd_tx, cmd_rx) = std_mpsc::channel();
    let (ready_tx, ready_rx) = worker_result_channel();

    let module_uid_for_thread = module_uid.clone();
    let worker = spawn_tt_thread("wasm-session".to_string(), move || {
        run_session_worker(
            module_uid_for_thread,
            PreparedComponent {
                component,
                instance_pre,
            },
            cfg,
            fuel,
            export_cache,
            ready_tx,
            cmd_rx,
        )
    })?;

    if let Err(err) = ready_rx.recv() {
        let _ = join_handle_via_coio(worker);
        return Err(err);
    }
    if let Err(err) = lifecycle::ensure_running() {
        let (done_tx, _done_rx) = worker_result_channel();
        let _ = cmd_tx.send(SessionCommand::Close(done_tx));
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

pub(crate) fn tt_wasm_bind_instance(module_uid: String) -> Result<WasmInstanceTarget> {
    lifecycle::ensure_running()?;
    let mode = runtime_state
        .modules
        .read()
        .unwrap()
        .get(&module_uid)
        .ok_or_else(|| anyhow!("module `{module_uid}` not found"))?
        .mode
        .clone();
    match mode {
        ModuleMode::Regular => Ok(WasmInstanceTarget::Session(tt_wasm_open_session(
            module_uid, None,
        )?)),
        ModuleMode::Interpreter(entry) => Ok(WasmInstanceTarget::Interpreter(entry)),
    }
}

pub(crate) fn tt_wasm_instance_call_function_with_ownership(
    target: &WasmInstanceTarget,
    interface: Option<String>,
    func: String,
    args: Vec<RmpvValue>,
) -> Result<SessionCallResult> {
    target.ensure_not_reentrant()?;
    match target {
        WasmInstanceTarget::Session(session_handle) => {
            tt_wasm_session_call_function_with_ownership(
                session_handle.clone(),
                interface,
                func,
                args,
            )
        }
        WasmInstanceTarget::Interpreter(entry) => {
            tt_wasm_interpreter_call_export_with_ownership(entry, interface, func, args)
        }
    }
}

pub(crate) fn tt_wasm_instance_drop_resource(target: &WasmInstanceTarget, id: u64) -> Result<()> {
    target.ensure_not_reentrant()?;
    match target {
        WasmInstanceTarget::Session(session_handle) => {
            tt_wasm_session_drop_resource(session_handle.clone(), id)
        }
        WasmInstanceTarget::Interpreter(entry) => tt_wasm_interpreter_drop_resource(entry, id),
    }
}

pub(crate) fn tt_wasm_instance_drop_resource_detached(target: &WasmInstanceTarget, id: u64) {
    match target {
        WasmInstanceTarget::Session(session_handle) => {
            tt_wasm_session_drop_resource_detached(session_handle, id);
        }
        WasmInstanceTarget::Interpreter(entry) => {
            tt_wasm_interpreter_drop_resource_detached(entry, id);
        }
    }
}

pub(crate) fn tt_wasm_close_instance_detached(target: &WasmInstanceTarget) {
    if let WasmInstanceTarget::Session(session_handle) = target {
        tt_wasm_close_session_detached(session_handle);
    }
}

/// Call a single export through an opened long-lived session.
#[allow(dead_code)]
pub fn tt_wasm_session_call_function(
    session_handle: String,
    interface: Option<String>,
    func: String,
    args: Vec<RmpvValue>,
) -> Result<RmpvValue> {
    Ok(tt_wasm_session_call_function_with_ownership(session_handle, interface, func, args)?.value)
}

pub(crate) fn start_wasm_session_call_function_with_ownership(
    session_handle: String,
    interface: Option<String>,
    func: String,
    args: Vec<RmpvValue>,
) -> Result<WorkerResultReceiver<SessionCallResult>> {
    lifecycle::ensure_running()?;
    let caller_session_id = current_session_id();
    let session = runtime_state
        .sessions
        .read()
        .unwrap()
        .get(&session_handle)
        .cloned()
        .ok_or_else(|| anyhow!("session `{session_handle}` not found"))?;
    let call_chain = ensure_target_not_active(
        session.worker_thread_id()?,
        &format!("session `{session_handle}`"),
    )?;

    let (result_tx, result_rx) = worker_result_channel();
    session
        .cmd_tx
        .send(SessionCommand::Call(
            CallJob::new(interface, func, args)
                .with_caller_session_id(caller_session_id)
                .with_call_chain(call_chain),
            result_tx,
        ))
        .map_err(|_| anyhow!("session `{session_handle}` is closed"))?;

    Ok(result_rx)
}

pub(crate) fn tt_wasm_session_call_function_with_ownership(
    session_handle: String,
    interface: Option<String>,
    func: String,
    args: Vec<RmpvValue>,
) -> Result<SessionCallResult> {
    start_wasm_session_call_function_with_ownership(session_handle, interface, func, args)?.recv()
}

/// Call multiple exports through an opened long-lived session.
#[allow(dead_code)]
pub fn tt_wasm_session_call_functions(
    session_handle: String,
    calls: Vec<(Option<String>, String, Vec<RmpvValue>)>,
) -> Result<Vec<RmpvValue>> {
    Ok(
        tt_wasm_session_call_functions_with_ownership(session_handle, calls)?
            .into_iter()
            .map(|result| result.value)
            .collect(),
    )
}

pub(crate) fn start_wasm_session_call_functions_with_ownership(
    session_handle: String,
    calls: Vec<(Option<String>, String, Vec<RmpvValue>)>,
) -> Result<WorkerResultReceiver<Vec<SessionCallResult>>> {
    lifecycle::ensure_running()?;
    let caller_session_id = current_session_id();
    let session = runtime_state
        .sessions
        .read()
        .unwrap()
        .get(&session_handle)
        .cloned()
        .ok_or_else(|| anyhow!("session `{session_handle}` not found"))?;
    let call_chain = ensure_target_not_active(
        session.worker_thread_id()?,
        &format!("session `{session_handle}`"),
    )?;

    let jobs = calls
        .into_iter()
        .map(|call| CallJob::from(call).with_caller_session_id(caller_session_id))
        .map(|job| job.with_call_chain(call_chain.clone()))
        .collect::<Vec<_>>();
    let (result_tx, result_rx) = worker_result_channel();
    session
        .cmd_tx
        .send(SessionCommand::Batch(jobs, result_tx))
        .map_err(|_| anyhow!("session `{session_handle}` is closed"))?;

    Ok(result_rx)
}

pub(crate) fn tt_wasm_session_call_functions_with_ownership(
    session_handle: String,
    calls: Vec<(Option<String>, String, Vec<RmpvValue>)>,
) -> Result<Vec<SessionCallResult>> {
    start_wasm_session_call_functions_with_ownership(session_handle, calls)?.recv()
}

pub fn tt_wasm_session_drop_resource(session_handle: String, id: u64) -> Result<()> {
    lifecycle::ensure_running()?;
    let session = runtime_state
        .sessions
        .read()
        .unwrap()
        .get(&session_handle)
        .cloned()
        .ok_or_else(|| anyhow!("session `{session_handle}` not found"))?;
    ensure_target_not_active(
        session.worker_thread_id()?,
        &format!("session `{session_handle}`"),
    )?;
    let (result_tx, result_rx) = worker_result_channel();
    session
        .cmd_tx
        .send(SessionCommand::DropResource(id, Some(result_tx)))
        .map_err(|_| anyhow!("session `{session_handle}` is closed"))?;
    result_rx.recv()
}

pub fn tt_wasm_session_drop_resource_detached(session_handle: &str, id: u64) {
    let session = runtime_state
        .sessions
        .read()
        .unwrap()
        .get(session_handle)
        .cloned();
    if let Some(session) = session {
        let _ = session.cmd_tx.send(SessionCommand::DropResource(id, None));
    }
}

/// Close a long-lived session and release its worker thread and instance.
pub fn tt_wasm_close_session(session_handle: String) -> Result<()> {
    lifecycle::ensure_running()?;
    let session = runtime_state
        .sessions
        .write()
        .unwrap()
        .remove(&session_handle)
        .ok_or_else(|| anyhow!("session `{session_handle}` not found"))?;

    close_session_impl(&session_handle, session)
}

pub fn tt_wasm_close_session_detached(session_handle: &str) {
    let session = runtime_state
        .sessions
        .write()
        .unwrap()
        .remove(session_handle);
    if let Some(session) = session {
        let (done_tx, _done_rx) = worker_result_channel();
        let _ = session.cmd_tx.send(SessionCommand::Close(done_tx));
        if let Some(handle) = session.join_handle.lock().unwrap().take() {
            let mut workers = DETACHED_SESSION_WORKERS.lock().unwrap();
            let mut pending = Vec::with_capacity(workers.len() + 1);
            for worker in workers.drain(..) {
                if worker.1.is_finished() {
                    let _ = worker.1.join();
                } else {
                    pending.push(worker);
                }
            }
            pending.push((session_handle.to_string(), handle));
            *workers = pending;
        }
    }
}
