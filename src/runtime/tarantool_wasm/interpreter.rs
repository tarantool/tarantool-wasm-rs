use super::*;

pub(super) struct SharedInterpreterEntry {
    pub(super) path_key: String,
    pub(super) component: Component,
    pub(super) config: WasmConfig,
    pub(super) fuel: bool,
    pub(super) export_cache: Arc<RwLock<HashMap<ExportCacheKey, ComponentExportIndex>>>,
    pub(super) cmd_tx: std_mpsc::Sender<InterpreterCommand>,
    pub(super) join_handle: Mutex<Option<JoinHandle<Result<()>>>>,
    pub(super) refcount: AtomicU64,
}

pub(super) enum InterpreterCommand {
    Exec(String, std_mpsc::Sender<Result<()>>),
    Call {
        func: String,
        args_msgpack: Vec<u8>,
        result_tx: std_mpsc::Sender<Result<Vec<u8>>>,
    },
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

fn run_interpreter_exec_job(ctx: InterpreterJobCtx<'_>, code: String) -> Result<()> {
    reset_store_fuel(ctx.store, ctx.with_fuel, ctx.config);
    ctx.rt
        .block_on(async {
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
    ctx.rt
        .block_on(async {
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
    let mut store = init_store(engine, config, with_fuel)?;
    let instance = rt
        .block_on(linker.instantiate_async(&mut store, component))
        .map_err(|err| anyhow!("failed to instantiate interpreter `{path_key}`: {err}"))?;
    Ok((store, instance))
}

pub(super) fn run_interpreter_worker(
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

/// Execute source code in a shared interpreter instance.
pub fn tt_wasm_interpreter_exec(module_uid: String, code: String) -> Result<()> {
    lifecycle::ensure_running()?;
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
    lifecycle::ensure_running()?;
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
