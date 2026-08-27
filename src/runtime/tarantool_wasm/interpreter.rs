use super::*;

pub(crate) struct SharedInterpreterEntry {
    pub(super) path_key: String,
    pub(super) component: Component,
    pub(super) instance_pre: InstancePre<SyncWasmStates>,
    pub(super) config: WasmConfig,
    pub(super) fuel: bool,
    pub(super) export_cache: Arc<RwLock<HashMap<ExportCacheKey, ComponentExportIndex>>>,
    pub(super) cmd_tx: std_mpsc::Sender<InterpreterCommand>,
    pub(super) join_handle: Mutex<Option<JoinHandle<Result<()>>>>,
    pub(super) refcount: AtomicU64,
}

impl SharedInterpreterEntry {
    pub(super) fn worker_thread_id(&self) -> Result<ThreadId> {
        self.join_handle
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .as_ref()
            .map(|handle| handle.thread().id())
            .ok_or_else(|| anyhow!("shared interpreter worker is no longer available"))
    }

    fn ensure_not_reentrant(&self) -> Result<CallChain> {
        ensure_target_not_active(
            self.worker_thread_id()?,
            &format!("interpreter `{}`", self.path_key),
        )
    }
}

pub(super) enum InterpreterCommand {
    Exec {
        code: String,
        caller_session_id: u64,
        call_chain: CallChain,
        result_tx: std_mpsc::Sender<Result<()>>,
    },
    Call {
        func: String,
        args_msgpack: Vec<u8>,
        caller_session_id: u64,
        call_chain: CallChain,
        result_tx: std_mpsc::Sender<Result<Vec<u8>>>,
    },
    ExportCall(CallJob, std_mpsc::Sender<Result<SessionCallResult>>),
    DropResource(u64, Option<std_mpsc::Sender<Result<()>>>),
    Close(std_mpsc::Sender<Result<()>>),
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
    caller_session_id: u64,
    call_chain: CallChain,
) -> Result<Result<()>> {
    reset_store_fuel(ctx.store, ctx.with_fuel, ctx.config);
    let response = ctx.rt.block_on(async {
        let job = CallJob::new(None, "exec".to_string(), vec![RmpvValue::from(code)])
            .with_caller_session_id(caller_session_id)
            .with_call_chain(call_chain);
        execute_call_job_on_instance(
            ctx.store,
            ctx.instance,
            ctx.component,
            ctx.export_cache,
            &job,
        )
        .await
    })?;
    Ok(decode_exec_response(response))
}

fn run_interpreter_call_job(
    ctx: InterpreterJobCtx<'_>,
    func: String,
    args_msgpack: Vec<u8>,
    caller_session_id: u64,
    call_chain: CallChain,
) -> Result<Result<Vec<u8>>> {
    reset_store_fuel(ctx.store, ctx.with_fuel, ctx.config);
    let state = ctx.store.data_mut();
    state.set_caller_session_id(Some(caller_session_id));
    state.enter_call(call_chain)?;

    let call_job = CallJob::new(None, "call".to_string(), Vec::new());
    let call_index = resolve_call_export_index(ctx.component, ctx.export_cache, &call_job)?;
    let call = ctx
        .instance
        .get_typed_func::<(String, Vec<u8>), (Result<Vec<u8>, String>,)>(
            &mut *ctx.store,
            call_index,
        )?;

    let fuel_before = ctx.store.get_fuel().ok();
    let started = std::time::Instant::now();
    let call_result = ctx
        .rt
        .block_on(call.call_async(&mut *ctx.store, (func, args_msgpack)));
    metrics::observe_guest_execution(started.elapsed());
    metrics::observe_fuel(fuel_before, ctx.store.get_fuel().ok());
    if let Err(err) = &call_result {
        metrics::observe_trap(err);
    }

    let (result,) = call_result?;
    Ok(result.map_err(|err| anyhow!("interpreter `call` failed: {err}")))
}

struct InterpreterInstanceCtx<'a> {
    path_key: &'a str,
    engine: &'a Engine,
    component: &'a Component,
    instance_pre: &'a InstancePre<SyncWasmStates>,
    config: &'a WasmConfig,
    with_fuel: bool,
    rt: &'a tokio::runtime::Runtime,
}

fn instantiate_interpreter_instance(
    ctx: &InterpreterInstanceCtx<'_>,
    next_resource_id: u64,
) -> Result<(Store<SyncWasmStates>, Instance, SessionResources)> {
    let mut store = init_store(ctx.engine, ctx.config, ctx.with_fuel)?;
    let started = std::time::Instant::now();
    let instance = ctx
        .rt
        .block_on(ctx.instance_pre.instantiate_async(&mut store))
        .map_err(|err| {
            anyhow!(
                "failed to instantiate interpreter `{}`: {err}",
                ctx.path_key
            )
        })?;
    metrics::observe_instantiation(started.elapsed());
    let resources = SessionResources::new_with_next_id(
        ctx.engine,
        ctx.component,
        &instance,
        &mut store,
        next_resource_id,
    );
    Ok((store, instance, resources))
}

pub(super) fn run_interpreter_worker(
    path_key: String,
    prepared: PreparedComponent,
    config: WasmConfig,
    with_fuel: bool,
    export_cache: Arc<RwLock<HashMap<ExportCacheKey, ComponentExportIndex>>>,
    ready_tx: std_mpsc::Sender<Result<()>>,
    cmd_rx: std_mpsc::Receiver<InterpreterCommand>,
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
    let rt = Builder::new_current_thread()
        .enable_time()
        .enable_io()
        .build()
        .unwrap();

    let instance_ctx = InterpreterInstanceCtx {
        path_key: &path_key,
        engine,
        component: &component,
        instance_pre: &instance_pre,
        config: &config,
        with_fuel,
        rt: &rt,
    };
    let (mut store, mut instance, mut resources) =
        match instantiate_interpreter_instance(&instance_ctx, 1) {
            Ok(v) => v,
            Err(init_error) => {
                let _ = ready_tx.send(Err(anyhow!("{init_error}")));
                return Err(init_error);
            }
        };

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
            InterpreterCommand::Exec {
                code,
                caller_session_id,
                call_chain,
                result_tx,
            } => {
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
                    caller_session_id,
                    call_chain,
                );
                match result {
                    Ok(Ok(())) => {
                        let _ = result_tx.send(Ok(()));
                    }
                    Ok(Err(err)) => {
                        let _ = result_tx.send(Err(err));
                    }
                    Err(err) => {
                        match instantiate_interpreter_instance(&instance_ctx, resources.next_id()) {
                            Ok((new_store, new_instance, new_resources)) => {
                                store = new_store;
                                instance = new_instance;
                                resources = new_resources;
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
                caller_session_id,
                call_chain,
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
                    caller_session_id,
                    call_chain,
                );
                match result {
                    Ok(Ok(raw)) => {
                        let _ = result_tx.send(Ok(raw));
                    }
                    Ok(Err(err)) => {
                        let _ = result_tx.send(Err(err));
                    }
                    Err(err) => {
                        match instantiate_interpreter_instance(&instance_ctx, resources.next_id()) {
                            Ok((new_store, new_instance, new_resources)) => {
                                store = new_store;
                                instance = new_instance;
                                resources = new_resources;
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
            InterpreterCommand::ExportCall(job, result_tx) => {
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
                if result
                    .as_ref()
                    .err()
                    .is_some_and(|err| err.downcast_ref::<wasmtime::Trap>().is_some())
                {
                    match instantiate_interpreter_instance(&instance_ctx, resources.next_id()) {
                        Ok((new_store, new_instance, new_resources)) => {
                            store = new_store;
                            instance = new_instance;
                            resources = new_resources;
                        }
                        Err(recover_err) => {
                            let err = result.as_ref().err().expect("trap result must be an error");
                            let combined = anyhow!(
                                "{err}; interpreter `{path_key}` entered unrecoverable state: {recover_err}"
                            );
                            let _ = result_tx.send(Err(combined));
                            break;
                        }
                    }
                }
                let _ = result_tx.send(result);
            }
            InterpreterCommand::DropResource(id, result_tx) => {
                let result = rt.block_on(resources.drop_resource(&mut store, id));
                if let Some(result_tx) = result_tx {
                    let _ = result_tx.send(result);
                }
            }
            InterpreterCommand::Close(done_tx) => {
                let result = rt.block_on(resources.drop_all(&mut store));
                let _ = done_tx.send(result);
                break;
            }
        }
    }

    Ok(())
}

pub(super) fn close_interpreter_impl(
    path_key: &str,
    entry: Arc<SharedInterpreterEntry>,
) -> Result<()> {
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

/// Return the canonical path used to identify a shared interpreter.
pub fn tt_wasm_interpreter_key(module_uid: String) -> Result<String> {
    lifecycle::ensure_running()?;
    let entry = interpreter_entry_for_module(&module_uid)?;
    entry.ensure_not_reentrant()?;
    Ok(entry.path_key.clone())
}

/// Execute source code in a shared interpreter instance.
pub fn tt_wasm_interpreter_exec(module_uid: String, code: String) -> Result<()> {
    lifecycle::ensure_running()?;
    let caller_session_id = current_session_id();
    let entry = interpreter_entry_for_module(&module_uid)?;
    let call_chain = entry.ensure_not_reentrant()?;
    let (result_tx, result_rx) = std_mpsc::channel();
    entry
        .cmd_tx
        .send(InterpreterCommand::Exec {
            code,
            caller_session_id,
            call_chain,
            result_tx,
        })
        .map_err(|_| anyhow!("interpreter for module `{module_uid}` is closed"))?;

    recv_result_via_coio(result_rx)
}

/// Call a function in a shared interpreter instance.
pub fn tt_wasm_interpreter_call(
    module_uid: String,
    func: String,
    args: Vec<RmpvValue>,
) -> Result<Vec<u8>> {
    lifecycle::ensure_running()?;
    let caller_session_id = current_session_id();
    let entry = interpreter_entry_for_module(&module_uid)?;
    let call_chain = entry.ensure_not_reentrant()?;
    let args_msgpack = rmpv_to_msgpack_bytes(&RmpvValue::Array(args))?;
    let (result_tx, result_rx) = std_mpsc::channel();

    entry
        .cmd_tx
        .send(InterpreterCommand::Call {
            func,
            args_msgpack,
            caller_session_id,
            call_chain,
            result_tx,
        })
        .map_err(|_| anyhow!("interpreter for module `{module_uid}` is closed"))?;

    recv_result_via_coio(result_rx)
}

pub(crate) fn tt_wasm_interpreter_call_export_with_ownership(
    entry: &Arc<SharedInterpreterEntry>,
    interface: Option<String>,
    func: String,
    args: Vec<RmpvValue>,
) -> Result<SessionCallResult> {
    lifecycle::ensure_running()?;
    let caller_session_id = current_session_id();
    let call_chain = entry.ensure_not_reentrant()?;
    let (result_tx, result_rx) = std_mpsc::channel();
    entry
        .cmd_tx
        .send(InterpreterCommand::ExportCall(
            CallJob::new(interface, func, args)
                .with_caller_session_id(caller_session_id)
                .with_call_chain(call_chain),
            result_tx,
        ))
        .map_err(|_| anyhow!("interpreter `{}` is closed", entry.path_key))?;
    recv_result_via_coio(result_rx)
}

pub(crate) fn tt_wasm_interpreter_drop_resource(
    entry: &Arc<SharedInterpreterEntry>,
    id: u64,
) -> Result<()> {
    lifecycle::ensure_running()?;
    entry.ensure_not_reentrant()?;
    let (result_tx, result_rx) = std_mpsc::channel();
    entry
        .cmd_tx
        .send(InterpreterCommand::DropResource(id, Some(result_tx)))
        .map_err(|_| anyhow!("interpreter `{}` is closed", entry.path_key))?;
    recv_result_via_coio(result_rx)
}

pub(crate) fn tt_wasm_interpreter_drop_resource_detached(
    entry: &Arc<SharedInterpreterEntry>,
    id: u64,
) {
    let _ = entry
        .cmd_tx
        .send(InterpreterCommand::DropResource(id, None));
}
