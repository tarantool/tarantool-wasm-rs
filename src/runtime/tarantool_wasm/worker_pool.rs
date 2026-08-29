use std::{
    any::Any,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::atomic::AtomicUsize,
};

use tokio::runtime::Runtime;

use super::{registry::LoadedModule, *};

const MIN_STATELESS_WORKERS: usize = 2;
const MAX_STATELESS_WORKERS: usize = 32;
pub(super) const STATELESS_WORKER_QUEUE_CAPACITY: usize = 32;

pub(super) struct StatelessExecution {
    engine: Engine,
    component: Component,
    instance_pre: InstancePre<SyncWasmStates>,
    config: WasmConfig,
    fuel: bool,
    export_cache: Arc<ExportCache>,
}

impl StatelessExecution {
    pub(super) fn new(loaded: LoadedModule) -> Self {
        let engine = if loaded.fuel {
            runtime_state.fuel_engine.clone()
        } else {
            runtime_state.engine.clone()
        };
        Self {
            engine,
            component: loaded.component,
            instance_pre: loaded.instance_pre,
            config: loaded.config,
            fuel: loaded.fuel,
            export_cache: loaded.export_cache,
        }
    }
}

enum StatelessCommand {
    Call {
        execution: StatelessExecution,
        job: CallJob,
        result_tx: WorkerResultSender<RmpvValue>,
        submitted_at: std::time::Instant,
    },
    Batch {
        execution: StatelessExecution,
        jobs: Vec<CallJob>,
        result_tx: WorkerResultSender<Vec<RmpvValue>>,
        submitted_at: std::time::Instant,
    },
}

struct PoolWorker {
    thread_id: ThreadId,
    command_tx: Mutex<Option<std_mpsc::SyncSender<StatelessCommand>>>,
    online: Arc<AtomicBool>,
    join_handle: Mutex<Option<JoinHandle<Result<()>>>>,
}

pub(super) struct StatelessWorkerPool {
    workers: Vec<PoolWorker>,
    next_worker: AtomicUsize,
    accepting: Arc<AtomicBool>,
}

impl StatelessWorkerPool {
    pub(super) fn new() -> Result<Self> {
        let worker_count = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(MIN_STATELESS_WORKERS)
            .clamp(MIN_STATELESS_WORKERS, MAX_STATELESS_WORKERS);
        let mut workers = Vec::with_capacity(worker_count);
        let accepting = Arc::new(AtomicBool::new(true));

        for index in 0..worker_count {
            let (command_tx, command_rx) = std_mpsc::sync_channel(STATELESS_WORKER_QUEUE_CAPACITY);
            let (ready_tx, ready_rx) = std_mpsc::sync_channel(1);
            let online = Arc::new(AtomicBool::new(true));
            let worker_online = Arc::clone(&online);
            let worker_accepting = Arc::clone(&accepting);
            let name = format!("wasm-executor-{index}");
            let join_handle = match spawn_tt_thread(name.clone(), move || {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    run_worker(command_rx, ready_tx, worker_accepting)
                }));
                worker_online.store(false, Ordering::Release);
                result.unwrap_or_else(|panic| {
                    Err(anyhow!(
                        "stateless WASM worker panicked: {}",
                        panic_message(panic.as_ref())
                    ))
                })
            }) {
                Ok(join_handle) => join_handle,
                Err(err) => {
                    stop_started_workers(&workers);
                    return Err(err).with_context(|| {
                        format!("failed to start stateless WASM worker `{name}`")
                    });
                }
            };

            let thread_id = match ready_rx.recv() {
                Ok(Ok(thread_id)) => thread_id,
                Ok(Err(message)) => {
                    drop(command_tx);
                    let _ = join_handle.join();
                    stop_started_workers(&workers);
                    return Err(anyhow!(message));
                }
                Err(_) => {
                    drop(command_tx);
                    let joined = join_handle.join();
                    stop_started_workers(&workers);
                    return Err(match joined {
                        Ok(Err(err)) => err,
                        Ok(Ok(())) => {
                            anyhow!("stateless WASM worker `{name}` stopped during startup")
                        }
                        Err(panic) => anyhow!(
                            "stateless WASM worker `{name}` panicked during startup: {}",
                            panic_message(panic.as_ref())
                        ),
                    });
                }
            };
            workers.push(PoolWorker {
                thread_id,
                command_tx: Mutex::new(Some(command_tx)),
                online,
                join_handle: Mutex::new(Some(join_handle)),
            });
        }

        Ok(Self {
            workers,
            next_worker: AtomicUsize::new(0),
            accepting,
        })
    }

    pub(super) fn submit_call(
        &self,
        execution: StatelessExecution,
        job: CallJob,
        result_tx: WorkerResultSender<RmpvValue>,
    ) -> Result<()> {
        let call_chain = job.call_chain.clone();
        self.submit(
            &call_chain,
            StatelessCommand::Call {
                execution,
                job,
                result_tx,
                submitted_at: std::time::Instant::now(),
            },
        )
    }

    pub(super) fn submit_batch(
        &self,
        execution: StatelessExecution,
        jobs: Vec<CallJob>,
        result_tx: WorkerResultSender<Vec<RmpvValue>>,
    ) -> Result<()> {
        let call_chain = jobs
            .first()
            .map(|job| job.call_chain.clone())
            .unwrap_or_default();
        self.submit(
            &call_chain,
            StatelessCommand::Batch {
                execution,
                jobs,
                result_tx,
                submitted_at: std::time::Instant::now(),
            },
        )
    }

    fn submit(&self, call_chain: &CallChain, command: StatelessCommand) -> Result<()> {
        if !self.accepting.load(Ordering::Acquire) {
            return Err(anyhow!("stateless WASM worker pool is stopping"));
        }

        let worker_count = self.workers.len();
        let start = self.next_worker.fetch_add(1, Ordering::Relaxed) % worker_count;
        let mut command = Some(command);
        let mut eligible = 0;
        let mut saturated = 0;

        for offset in 0..worker_count {
            let worker = &self.workers[(start + offset) % worker_count];
            if call_chain.contains(&worker.thread_id) {
                continue;
            }
            eligible += 1;
            if !worker.online.load(Ordering::Acquire) {
                continue;
            }

            let command_tx = worker.command_tx.lock().unwrap();
            let Some(command_tx) = command_tx.as_ref() else {
                continue;
            };
            match command_tx.try_send(command.take().unwrap()) {
                Ok(()) => return Ok(()),
                Err(std_mpsc::TrySendError::Full(returned)) => {
                    command = Some(returned);
                    saturated += 1;
                }
                Err(std_mpsc::TrySendError::Disconnected(returned)) => {
                    worker.online.store(false, Ordering::Release);
                    command = Some(returned);
                }
            }
        }

        if eligible == 0 {
            return Err(anyhow!(
                "no stateless WASM worker is available outside the active call chain (pool size {worker_count})"
            ));
        }
        if saturated != 0 {
            return Err(anyhow!(
                "stateless WASM worker pool is saturated ({worker_count} workers, queue capacity {} per worker)",
                STATELESS_WORKER_QUEUE_CAPACITY
            ));
        }
        Err(anyhow!("stateless WASM worker pool has no live workers"))
    }

    pub(super) fn shutdown(&self) {
        self.accepting.store(false, Ordering::Release);
        for worker in &self.workers {
            worker.command_tx.lock().unwrap().take();
        }
    }

    pub(super) fn join(&self, errors: &mut Vec<String>) {
        for worker in &self.workers {
            let Some(join_handle) = worker.join_handle.lock().unwrap().take() else {
                continue;
            };
            match join_handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(err)) => errors.push(format!(
                    "stateless WASM worker {:?} failed: {err}",
                    worker.thread_id
                )),
                Err(panic) => errors.push(format!(
                    "stateless WASM worker {:?} panicked: {}",
                    worker.thread_id,
                    panic_message(panic.as_ref())
                )),
            }
        }
    }
}

fn run_worker(
    command_rx: std_mpsc::Receiver<StatelessCommand>,
    ready_tx: std_mpsc::SyncSender<std::result::Result<ThreadId, String>>,
    accepting: Arc<AtomicBool>,
) -> Result<()> {
    let mut runtime = match build_worker_runtime() {
        Ok(runtime) => runtime,
        Err(err) => {
            let message = format!("failed to initialize stateless WASM worker runtime: {err:#}");
            let _ = ready_tx.send(Err(message.clone()));
            return Err(anyhow!(message));
        }
    };
    ready_tx
        .send(Ok(thread::current().id()))
        .map_err(|_| anyhow!("stateless WASM worker startup receiver was dropped"))?;
    let mut scratch = CallScratch::default();

    while let Ok(command) = command_rx.recv() {
        if !accepting.load(Ordering::Acquire) {
            reject_queued_command(command);
            continue;
        }
        let panicked = match command {
            StatelessCommand::Call {
                execution,
                job,
                result_tx,
                submitted_at,
            } => {
                metrics::observe_worker_start(submitted_at.elapsed());
                let (result, panicked) = catch_job_panic(|| {
                    execute_stateless_call(&runtime, execution, job, &mut scratch)
                });
                let _ = result_tx.send(result);
                panicked
            }
            StatelessCommand::Batch {
                execution,
                jobs,
                result_tx,
                submitted_at,
            } => {
                metrics::observe_worker_start(submitted_at.elapsed());
                let (result, panicked) = catch_job_panic(|| {
                    execute_stateless_batch(&runtime, execution, jobs, &mut scratch)
                });
                let _ = result_tx.send(result);
                panicked
            }
        };
        if panicked {
            scratch = CallScratch::default();
            runtime = build_worker_runtime()
                .context("failed to rebuild stateless WASM worker runtime after panic")?;
        }
    }
    Ok(())
}

fn reject_queued_command(command: StatelessCommand) {
    match command {
        StatelessCommand::Call { result_tx, .. } => {
            let _ = result_tx.send(Err(anyhow!("WASM runtime is stopping")));
        }
        StatelessCommand::Batch { result_tx, .. } => {
            let _ = result_tx.send(Err(anyhow!("WASM runtime is stopping")));
        }
    }
}

fn stop_started_workers(workers: &[PoolWorker]) {
    for worker in workers {
        worker.command_tx.lock().unwrap().take();
    }
    for worker in workers {
        if let Some(join_handle) = worker.join_handle.lock().unwrap().take() {
            let _ = join_handle.join();
        }
    }
}

fn build_worker_runtime() -> Result<Runtime> {
    let started = std::time::Instant::now();
    let runtime = Builder::new_current_thread()
        .enable_time()
        .enable_io()
        .build()
        .context("failed to build stateless WASM worker runtime");
    metrics::observe_runtime_setup(started.elapsed());
    runtime
}

fn execute_stateless_call(
    runtime: &Runtime,
    execution: StatelessExecution,
    job: CallJob,
    scratch: &mut CallScratch,
) -> Result<RmpvValue> {
    let StatelessExecution {
        engine,
        component,
        instance_pre,
        config,
        fuel,
        export_cache,
    } = execution;
    runtime.block_on(async move {
        let mut store = init_store(&engine, &config, fuel)?;
        execute_call_job(
            &mut store,
            &instance_pre,
            &component,
            export_cache.as_ref(),
            &job,
            scratch,
        )
        .await
    })
}

fn execute_stateless_batch(
    runtime: &Runtime,
    execution: StatelessExecution,
    jobs: Vec<CallJob>,
    scratch: &mut CallScratch,
) -> Result<Vec<RmpvValue>> {
    let StatelessExecution {
        engine,
        component,
        instance_pre,
        config,
        fuel,
        export_cache,
    } = execution;
    runtime.block_on(async move {
        let mut results = Vec::with_capacity(jobs.len());
        for job in jobs {
            let mut store = init_store(&engine, &config, fuel)?;
            results.push(
                execute_call_job(
                    &mut store,
                    &instance_pre,
                    &component,
                    export_cache.as_ref(),
                    &job,
                    scratch,
                )
                .await?,
            );
        }
        Ok(results)
    })
}

fn catch_job_panic<T>(f: impl FnOnce() -> Result<T>) -> (Result<T>, bool) {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(result) => (result, false),
        Err(panic) => (
            Err(anyhow!(
                "stateless WASM worker job panicked: {}",
                panic_message(panic.as_ref())
            )),
            true,
        ),
    }
}

fn panic_message(panic: &(dyn Any + Send)) -> String {
    panic
        .downcast_ref::<&str>()
        .map(|message| (*message).to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic payload".to_string())
}
