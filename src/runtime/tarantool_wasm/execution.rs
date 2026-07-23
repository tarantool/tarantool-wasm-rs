use super::*;

pub(crate) struct WasmRunHandle {
    run_id: u64,
    entry: Arc<WasmRunEntry>,
}

impl WasmRunHandle {
    fn new(run_id: u64, entry: Arc<WasmRunEntry>) -> Self {
        Self { run_id, entry }
    }

    pub(crate) fn cancel(&self) -> Result<()> {
        let _ = self
            .entry
            .cancel_tx
            .blocking_send(WasmFiberStatus::Canceled);
        Ok(())
    }

    pub(crate) fn take_join_handle(&mut self) -> Result<JoinHandle<Result<()>>> {
        runtime_state.runs.write().unwrap().remove(&self.run_id);
        self.entry
            .join_handle
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| anyhow!("join handle already taken"))
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub(super) struct ExportCacheKey {
    pub(super) interface: Option<String>,
    pub(super) func: String,
}

#[derive(Clone, Debug)]
pub(super) struct CallJob {
    pub(super) interface: Option<String>,
    pub(super) func: String,
    pub(super) args: Vec<RmpvValue>,
    pub(super) caller_session_id: Option<u64>,
    pub(super) call_chain: CallChain,
}

impl CallJob {
    pub(super) fn new(interface: Option<String>, func: String, args: Vec<RmpvValue>) -> Self {
        Self {
            interface,
            func,
            args,
            caller_session_id: None,
            call_chain: Vec::new(),
        }
    }
    pub(super) fn with_caller_session_id(mut self, session_id: u64) -> Self {
        self.caller_session_id = Some(session_id);
        self
    }

    pub(super) fn with_call_chain(mut self, call_chain: CallChain) -> Self {
        self.call_chain = call_chain;
        self
    }

    pub(super) fn cache_key(&self) -> ExportCacheKey {
        ExportCacheKey {
            interface: self.interface.clone(),
            func: self.func.clone(),
        }
    }
}
pub(super) fn current_session_id() -> u64 {
    debug_assert_tx_thread();
    unsafe { ffi::session::box_session_id() }
}

impl From<(Option<String>, String, Vec<RmpvValue>)> for CallJob {
    fn from(value: (Option<String>, String, Vec<RmpvValue>)) -> Self {
        let (interface, func, args) = value;
        Self::new(interface, func, args)
    }
}

pub(super) struct WasmRunEntry {
    pub(super) cancel_tx: mpsc::Sender<WasmFiberStatus>,
    pub(super) join_handle: Mutex<Option<JoinHandle<Result<()>>>>,
}

pub(super) fn recv_result_via_coio<T: Send + 'static>(
    receiver: std_mpsc::Receiver<Result<T>>,
) -> Result<T> {
    debug_assert_tx_thread();
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

pub(super) fn join_handle_via_coio<T: Send + 'static>(handle: JoinHandle<Result<T>>) -> Result<T> {
    debug_assert_tx_thread();
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

/// Run a loaded module in a background thread, returning a join handle.
pub fn tt_wasm_run_module(module_uid: String, config: Option<WasmConfig>) -> Result<WasmRunHandle> {
    lifecycle::ensure_running()?;
    let loaded = runtime_state
        .modules
        .read()
        .unwrap()
        .get(&module_uid)
        .ok_or_else(|| anyhow::anyhow!("module `{}` not found", module_uid))?
        .clone();
    let instance_pre = loaded.instance_pre;
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
        let wasi_view = match SyncWasmStates::new(cfg.clone()) {
            Ok(wasi_view) => wasi_view,
            Err(err) => {
                let message = format!("{err:#}");
                let _ = canceler_tx.send(Err(anyhow!("{message}")));
                return Err(anyhow!(message));
            }
        };
        let mut store = Store::new(engine, wasi_view);
        store.limiter(|state| &mut state.limits);
        if fuel {
            let limit = cfg.max_instructions.unwrap_or(u64::MAX);
            let _ = store.set_fuel(limit);
        }

        store.set_epoch_deadline(1);
        store.epoch_deadline_trap();

        let fiber = WasmFiber::new(store, instance_pre)?;
        let _ = canceler_tx.send(Ok(fiber.canceler()));
        fiber.block_on()?;
        Ok::<(), anyhow::Error>(())
    })?;
    let cancel_tx = match canceler_rx.recv() {
        Ok(Ok(cancel_tx)) => cancel_tx,
        Ok(Err(err)) => {
            let _ = join_handle_via_coio(handle);
            return Err(err);
        }
        Err(_) => {
            return match join_handle_via_coio(handle) {
                Err(err) => Err(err),
                Ok(()) => Err(anyhow!("failed to receive fiber cancel handle")),
            };
        }
    };
    if let Err(err) = lifecycle::ensure_running() {
        let _ = cancel_tx.blocking_send(WasmFiberStatus::Canceled);
        let _ = join_handle_via_coio(handle);
        return Err(err);
    }
    let run_id = runtime_state.next_run_uid.fetch_add(1, Ordering::Relaxed);
    let entry = Arc::new(WasmRunEntry {
        cancel_tx,
        join_handle: Mutex::new(Some(handle)),
    });
    runtime_state
        .runs
        .write()
        .unwrap()
        .insert(run_id, Arc::clone(&entry));
    Ok(WasmRunHandle::new(run_id, entry))
}

/// Call a single exported function of a module.
pub fn tt_wasm_call_function(
    module_uid: String,
    interface: Option<String>,
    func: String,
    args: Vec<RmpvValue>,
) -> Result<RmpvValue> {
    lifecycle::ensure_running()?;
    let caller_session_id = current_session_id();
    let call_chain = current_call_chain();
    let loaded = runtime_state
        .modules
        .read()
        .unwrap()
        .get(&module_uid)
        .ok_or_else(|| anyhow::anyhow!("module `{}` not found", module_uid))?
        .clone();

    let component = loaded.component;
    let instance_pre = loaded.instance_pre;
    let cfg = loaded.config;
    let fuel = loaded.fuel;
    let export_cache = loaded.export_cache;

    let handle = spawn_tt_thread("wasm".to_string(), move || {
        let engine = if fuel {
            &runtime_state.fuel_engine
        } else {
            &runtime_state.engine
        };
        let wasi_view = SyncWasmStates::new(cfg.clone())?;
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
            let job = CallJob::new(interface, func, args)
                .with_caller_session_id(caller_session_id)
                .with_call_chain(call_chain);
            execute_call_job(
                &mut store,
                &instance_pre,
                &component,
                export_cache.as_ref(),
                &job,
            )
            .await
        })
    })?;

    let mut result: Result<RmpvValue> = Ok(RmpvValue::Nil);

    debug_assert_tx_thread();
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

/// Call multiple exported functions of a module in sequence on one worker thread.
///
/// Each call uses a fresh store and component instance. Use a session batch when
/// guest state must persist between calls.
pub fn tt_wasm_call_functions(
    module_uid: String,
    calls: Vec<(Option<String>, String, Vec<RmpvValue>)>,
) -> Result<Vec<RmpvValue>> {
    lifecycle::ensure_running()?;
    let caller_session_id = current_session_id();
    let call_chain = current_call_chain();
    let loaded = runtime_state
        .modules
        .read()
        .unwrap()
        .get(&module_uid)
        .ok_or_else(|| anyhow::anyhow!("module `{}` not found", module_uid))?
        .clone();

    let component = loaded.component;
    let instance_pre = loaded.instance_pre;
    let cfg = loaded.config;
    let fuel = loaded.fuel;
    let export_cache = loaded.export_cache;
    let jobs = calls
        .into_iter()
        .map(|call| CallJob::from(call).with_caller_session_id(caller_session_id))
        .map(|job| job.with_call_chain(call_chain.clone()))
        .collect::<Vec<_>>();

    let handle = spawn_tt_thread("wasm".to_string(), move || {
        let engine = if fuel {
            &runtime_state.fuel_engine
        } else {
            &runtime_state.engine
        };
        let rt = Builder::new_current_thread()
            .enable_time()
            .enable_io()
            .build()
            .unwrap();

        rt.block_on(async move {
            let mut results_all = Vec::new();

            for job in jobs.into_iter() {
                let wasi_view = SyncWasmStates::new(cfg.clone())?;
                let mut store = Store::new(engine, wasi_view);
                store.limiter(|state| &mut state.limits);
                if fuel {
                    let limit = cfg.max_instructions.unwrap_or(u64::MAX);
                    let _ = store.set_fuel(limit);
                }

                store.set_epoch_deadline(1);
                store.epoch_deadline_trap();

                results_all.push(
                    execute_call_job(
                        &mut store,
                        &instance_pre,
                        &component,
                        export_cache.as_ref(),
                        &job,
                    )
                    .await?,
                );
            }

            Ok(results_all)
        })
    })?;

    let mut result: Result<Vec<RmpvValue>> = Ok(Vec::new());

    debug_assert_tx_thread();
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
