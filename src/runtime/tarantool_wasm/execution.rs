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
        self.entry.cancel_tx.request();
        Ok(())
    }

    pub(crate) fn ready_or_subscribe(&self) -> Result<bool> {
        self.entry.completion.ready_or_subscribe()
    }

    pub(crate) fn join(&mut self) -> Result<()> {
        let handle = self.take_join_handle()?;
        handle
            .join()
            .unwrap_or_else(|_| Err(anyhow!("thread panicked")))
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

pub(crate) struct WasmCallHandle<T> {
    receiver: Option<WorkerResultReceiver<T>>,
    join_handle: Option<JoinHandle<()>>,
}

impl<T> WasmCallHandle<T> {
    fn new(receiver: WorkerResultReceiver<T>, join_handle: JoinHandle<()>) -> Self {
        Self {
            receiver: Some(receiver),
            join_handle: Some(join_handle),
        }
    }

    pub(crate) fn join(mut self) -> Result<T> {
        let receiver = self
            .receiver
            .take()
            .ok_or_else(|| anyhow!("call result already taken"))?;
        let result = receiver.recv_ready();
        if let Some(handle) = self.join_handle.take().filter(JoinHandle::is_finished) {
            handle.join().map_err(|_| anyhow!("thread panicked"))?;
        }
        result
    }

    pub(crate) fn ready_or_subscribe(&self) -> Result<bool> {
        self.receiver
            .as_ref()
            .ok_or_else(|| anyhow!("call result already taken"))?
            .ready_or_subscribe()
    }
}

pub(super) struct WorkerCompletion {
    finished: AtomicBool,
    waiter: Mutex<Option<fiber::FiberId>>,
}

impl WorkerCompletion {
    fn new() -> Self {
        Self {
            finished: AtomicBool::new(false),
            waiter: Mutex::new(None),
        }
    }

    fn ready_or_subscribe(&self) -> Result<bool> {
        debug_assert_tx_thread();
        let mut waiter = self.waiter.lock().unwrap();
        if self.finished.load(Ordering::Acquire) {
            return Ok(true);
        }
        let current = fiber::id();
        match *waiter {
            None => *waiter = Some(current),
            Some(existing) if existing == current => {}
            Some(_) => return Err(anyhow!("worker result is already awaited by another fiber")),
        }
        Ok(false)
    }

    fn unsubscribe_current(&self) -> Result<()> {
        debug_assert_tx_thread();
        let mut waiter = self.waiter.lock().unwrap();
        let current = fiber::id();
        match *waiter {
            Some(existing) if existing == current => *waiter = None,
            None => {}
            Some(_) => return Err(anyhow!("worker result is awaited by another fiber")),
        }
        Ok(())
    }

    fn finish(&self) {
        self.finished.store(true, Ordering::Release);
        let waiter = self.waiter.lock().unwrap().take();
        if let Some(waiter) = waiter {
            run_on_tx_detached(move || {
                let _ = fiber::wakeup(waiter);
            });
        }
    }
}

struct CompletionNotifier(Arc<WorkerCompletion>);

impl Drop for CompletionNotifier {
    fn drop(&mut self) {
        self.0.finish();
    }
}

pub(super) struct WorkerResultSender<T> {
    sender: Option<std_mpsc::Sender<Result<T>>>,
    completion: Arc<WorkerCompletion>,
}

impl<T> WorkerResultSender<T> {
    pub(super) fn send(
        mut self,
        result: Result<T>,
    ) -> std::result::Result<(), std_mpsc::SendError<Result<T>>> {
        self.sender
            .take()
            .expect("worker result sender must be available")
            .send(result)
    }
}

impl<T> Drop for WorkerResultSender<T> {
    fn drop(&mut self) {
        drop(self.sender.take());
        self.completion.finish();
    }
}

pub(crate) struct WorkerResultReceiver<T> {
    receiver: std_mpsc::Receiver<Result<T>>,
    completion: Arc<WorkerCompletion>,
    started: std::time::Instant,
}

impl<T> WorkerResultReceiver<T> {
    pub(super) fn recv(self) -> Result<T>
    where
        T: Send + 'static,
    {
        debug_assert_tx_thread();
        let mut result: Result<T> = Err(anyhow!("failed to receive worker response"));
        coio_call(
            &mut |receiver: Box<std_mpsc::Receiver<Result<T>>>| {
                result = receiver
                    .recv()
                    .unwrap_or_else(|_| Err(anyhow!("worker stopped unexpectedly")));
                0
            },
            self.receiver,
        );
        metrics::observe_worker_wait(self.started.elapsed());
        result
    }

    pub(crate) fn ready_or_subscribe(&self) -> Result<bool> {
        self.completion.ready_or_subscribe()
    }

    pub(crate) fn unsubscribe_current(&self) -> Result<()> {
        self.completion.unsubscribe_current()
    }

    pub(crate) fn recv_ready(self) -> Result<T> {
        debug_assert_tx_thread();
        let ready = if self.completion.finished.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(anyhow!("worker response is not ready"))
        };
        let result = ready.and_then(|()| {
            self.receiver
                .recv()
                .unwrap_or_else(|_| Err(anyhow!("worker stopped unexpectedly")))
        });
        metrics::observe_worker_wait(self.started.elapsed());
        result
    }
}

pub(super) fn worker_result_channel<T>() -> (WorkerResultSender<T>, WorkerResultReceiver<T>) {
    let (sender, receiver) = std_mpsc::channel();
    let completion = Arc::new(WorkerCompletion::new());
    (
        WorkerResultSender {
            sender: Some(sender),
            completion: Arc::clone(&completion),
        },
        WorkerResultReceiver {
            receiver,
            completion,
            started: std::time::Instant::now(),
        },
    )
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
    pub(super) cancel_tx: WasmFiberCanceler,
    pub(super) join_handle: Mutex<Option<JoinHandle<Result<()>>>>,
    completion: Arc<WorkerCompletion>,
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
    let completion = Arc::new(WorkerCompletion::new());
    let worker_completion = Arc::clone(&completion);
    let handle = spawn_tt_thread("wasm".to_string(), move || {
        let _completion = CompletionNotifier(worker_completion);
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
        cancel_tx.request();
        let _ = join_handle_via_coio(handle);
        return Err(err);
    }
    let run_id = runtime_state.next_run_uid.fetch_add(1, Ordering::Relaxed);
    let entry = Arc::new(WasmRunEntry {
        cancel_tx,
        join_handle: Mutex::new(Some(handle)),
        completion,
    });
    runtime_state
        .runs
        .write()
        .unwrap()
        .insert(run_id, Arc::clone(&entry));
    Ok(WasmRunHandle::new(run_id, entry))
}

/// Start a single exported function call without waiting on the Tarantool thread.
pub(crate) fn start_wasm_call_function(
    module_uid: String,
    interface: Option<String>,
    func: String,
    args: Vec<RmpvValue>,
) -> Result<WasmCallHandle<RmpvValue>> {
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

    let (result_tx, result_rx) = worker_result_channel();
    let worker_started = std::time::Instant::now();
    let handle = spawn_tt_thread("wasm".to_string(), move || {
        let result = (|| {
            metrics::observe_worker_start(worker_started.elapsed());
            let engine = if fuel {
                &runtime_state.fuel_engine
            } else {
                &runtime_state.engine
            };
            let mut store = init_store(engine, &cfg, fuel)?;

            let runtime_started = std::time::Instant::now();
            let rt = Builder::new_current_thread()
                .enable_time()
                .enable_io()
                .build()
                .unwrap();
            metrics::observe_runtime_setup(runtime_started.elapsed());

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
        })();
        let _ = result_tx.send(result);
    })?;

    Ok(WasmCallHandle::new(result_rx, handle))
}

/// Start a batch of exported calls on one worker thread.
pub(crate) fn start_wasm_call_functions(
    module_uid: String,
    calls: Vec<(Option<String>, String, Vec<RmpvValue>)>,
) -> Result<WasmCallHandle<Vec<RmpvValue>>> {
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

    let (result_tx, result_rx) = worker_result_channel();
    let worker_started = std::time::Instant::now();
    let handle = spawn_tt_thread("wasm".to_string(), move || {
        let result = {
            metrics::observe_worker_start(worker_started.elapsed());
            let engine = if fuel {
                &runtime_state.fuel_engine
            } else {
                &runtime_state.engine
            };
            let runtime_started = std::time::Instant::now();
            let rt = Builder::new_current_thread()
                .enable_time()
                .enable_io()
                .build()
                .unwrap();
            metrics::observe_runtime_setup(runtime_started.elapsed());

            rt.block_on(async move {
                let mut results_all = Vec::new();

                for job in jobs.into_iter() {
                    let mut store = init_store(engine, &cfg, fuel)?;

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
        };
        let _ = result_tx.send(result);
    })?;

    Ok(WasmCallHandle::new(result_rx, handle))
}
