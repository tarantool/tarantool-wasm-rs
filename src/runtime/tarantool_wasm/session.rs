use super::*;

pub(super) enum SessionCommand {
    Call(CallJob, std_mpsc::Sender<Result<SessionCallResult>>),
    Batch(
        Vec<CallJob>,
        std_mpsc::Sender<Result<Vec<SessionCallResult>>>,
    ),
    DropResource(u64, Option<std_mpsc::Sender<Result<()>>>),
    Close(std_mpsc::Sender<Result<()>>),
}

pub(super) struct WasmSession {
    pub(super) module_uid: String,
    pub(super) cmd_tx: std_mpsc::Sender<SessionCommand>,
    pub(super) join_handle: Mutex<Option<JoinHandle<Result<()>>>>,
}

pub(super) fn run_session_worker(
    module_uid: String,
    prepared: PreparedComponent,
    config: WasmConfig,
    with_fuel: bool,
    export_cache: Arc<RwLock<HashMap<ExportCacheKey, ComponentExportIndex>>>,
    ready_tx: std_mpsc::Sender<Result<()>>,
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

    let rt = Builder::new_current_thread()
        .enable_time()
        .enable_io()
        .build()
        .unwrap();

    let instance = match rt.block_on(instance_pre.instantiate_async(&mut store)) {
        Ok(instance) => instance,
        Err(err) => {
            let init_error =
                anyhow!("failed to instantiate session for module `{module_uid}`: {err}");
            let _ = ready_tx.send(Err(anyhow!("{init_error}")));
            return Err(init_error);
        }
    };
    let mut resources = SessionResources::new(engine, &component, &instance, &mut store);

    let _ = ready_tx.send(Ok(()));

    loop {
        let command = match cmd_rx.recv_timeout(Duration::from_millis(10)) {
            Ok(command) => command,
            Err(std_mpsc::RecvTimeoutError::Timeout)
                if lifecycle::state() == RuntimeLifecycle::Running =>
            {
                continue;
            }
            Err(std_mpsc::RecvTimeoutError::Timeout | std_mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
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
    let (ready_tx, ready_rx) = std_mpsc::channel();

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

    if let Err(err) = recv_result_via_coio(ready_rx) {
        let _ = join_handle_via_coio(worker);
        return Err(err);
    }
    if let Err(err) = lifecycle::ensure_running() {
        let (done_tx, _done_rx) = std_mpsc::channel();
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

pub(crate) fn tt_wasm_session_call_function_with_ownership(
    session_handle: String,
    interface: Option<String>,
    func: String,
    args: Vec<RmpvValue>,
) -> Result<SessionCallResult> {
    lifecycle::ensure_running()?;
    let caller_session_id = current_session_id();
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
            CallJob::new(interface, func, args).with_caller_session_id(caller_session_id),
            result_tx,
        ))
        .map_err(|_| anyhow!("session `{session_handle}` is closed"))?;

    recv_result_via_coio(result_rx)
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

pub(crate) fn tt_wasm_session_call_functions_with_ownership(
    session_handle: String,
    calls: Vec<(Option<String>, String, Vec<RmpvValue>)>,
) -> Result<Vec<SessionCallResult>> {
    lifecycle::ensure_running()?;
    let caller_session_id = current_session_id();
    let session = runtime_state
        .sessions
        .read()
        .unwrap()
        .get(&session_handle)
        .cloned()
        .ok_or_else(|| anyhow!("session `{session_handle}` not found"))?;

    let jobs = calls
        .into_iter()
        .map(|call| CallJob::from(call).with_caller_session_id(caller_session_id))
        .collect::<Vec<_>>();
    let (result_tx, result_rx) = std_mpsc::channel();
    session
        .cmd_tx
        .send(SessionCommand::Batch(jobs, result_tx))
        .map_err(|_| anyhow!("session `{session_handle}` is closed"))?;

    recv_result_via_coio(result_rx)
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
    let (result_tx, result_rx) = std_mpsc::channel();
    session
        .cmd_tx
        .send(SessionCommand::DropResource(id, Some(result_tx)))
        .map_err(|_| anyhow!("session `{session_handle}` is closed"))?;
    recv_result_via_coio(result_rx)
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
        let (done_tx, _done_rx) = std_mpsc::channel();
        let _ = session.cmd_tx.send(SessionCommand::Close(done_tx));
    }
}
