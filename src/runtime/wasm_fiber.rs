use std::{
    future::{poll_fn, Future},
    mem,
    pin::Pin,
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc,
    },
    task::Poll,
    thread,
    time::Duration,
};

use anyhow::{anyhow, Result};
use tokio::{
    runtime::Builder,
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use wasmtime::{component::InstancePre, Engine, Store, UpdateDeadline};
use wasmtime_wasi::p2::bindings::CommandPre;
use wasmtime_wasi::I32Exit;

use crate::runtime::{
    lifecycle::{self, RuntimeLifecycle},
    metrics,
    tarantool_wasm::SyncWasmStates,
    tx::{tx_push, TxCtx},
};

struct SigAltStackRestoreGuard {
    previous: Option<libc::stack_t>,
}

fn handle_command_result(result: wasmtime::Result<Result<(), ()>>) -> Result<()> {
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(())) => Err(anyhow!("WASI command returned failure")),
        Err(err) => match err.downcast_ref::<I32Exit>() {
            Some(I32Exit(0)) => Ok(()),
            Some(I32Exit(status)) => Err(anyhow!("WASI command exited with status {status}")),
            None => {
                metrics::observe_trap(&err);
                Err(err.into())
            }
        },
    }
}

impl SigAltStackRestoreGuard {
    fn capture() -> Self {
        let mut previous = unsafe { mem::zeroed::<libc::stack_t>() };
        let captured = unsafe { libc::sigaltstack(ptr::null(), &mut previous) } == 0;
        if captured {
            Self {
                previous: Some(previous),
            }
        } else {
            Self { previous: None }
        }
    }
}

impl Drop for SigAltStackRestoreGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous {
            // Ignore failure intentionally: at this point we only try to keep
            // Tarantool's original alt stack for graceful shutdown.
            let _ = unsafe { libc::sigaltstack(&previous, ptr::null_mut()) };
        }
    }
}

const EXECUTION_RUNNING: u8 = 0;
const EXECUTION_CANCELED: u8 = 1;
const EXECUTION_COMPLETED: u8 = 2;

#[derive(Debug)]
pub(crate) struct EpochCancellation;

impl std::fmt::Display for EpochCancellation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WASM execution canceled")
    }
}

impl std::error::Error for EpochCancellation {}

/// Every store sharing an engine must extend its deadline on a pipe tick.
/// Shutdown remains global and takes precedence over local cancellation.
pub(crate) fn configure_epoch_deadline(
    store: &mut Store<SyncWasmStates>,
    canceled: Option<Arc<AtomicU8>>,
) {
    store.set_epoch_deadline(1);
    store.epoch_deadline_callback(move |_| {
        if lifecycle::state() != RuntimeLifecycle::Running {
            return Err(wasmtime::Error::msg("WASM runtime is stopping"));
        }
        if canceled
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Acquire) == EXECUTION_CANCELED)
        {
            return Err(EpochCancellation.into());
        }
        Ok(UpdateDeadline::Continue(1))
    });
}

struct ExecutionActive {
    state: Arc<AtomicU8>,
    active: Arc<AtomicBool>,
}

impl Drop for ExecutionActive {
    fn drop(&mut self) {
        // A completion and a cancellation request compete for one decision.
        let _ = self.state.compare_exchange(
            EXECUTION_RUNNING,
            EXECUTION_COMPLETED,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        self.active.store(false, Ordering::Release);
    }
}

pub(crate) enum WasmFiberStatus {
    Suspended,
    Finished,
    Canceled,
}

#[derive(Clone)]
pub(crate) struct WasmFiberCanceler {
    sender: mpsc::Sender<WasmFiberStatus>,
    state: Arc<AtomicU8>,
    engine: Option<Engine>,
    active: Arc<AtomicBool>,
    ticker_started: Arc<AtomicBool>,
}

impl WasmFiberCanceler {
    pub(crate) fn request(&self) {
        // The CAS linearizes cancel against task completion. A completed guest
        // keeps its original outcome, even when the two happen concurrently.
        if self
            .state
            .compare_exchange(
                EXECUTION_RUNNING,
                EXECUTION_CANCELED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            return;
        }
        let _ = self.sender.try_send(WasmFiberStatus::Canceled);
        if let Some(engine) = &self.engine {
            if self.active.load(Ordering::Acquire)
                && !self.ticker_started.swap(true, Ordering::AcqRel)
            {
                let engine = engine.clone();
                let active = Arc::clone(&self.active);
                // The ticker is independent of the Tokio worker, which may be
                // executing a CPU loop without yielding. Other stores rearm.
                let _ = thread::Builder::new()
                    .name("wasm-epoch-cancel".into())
                    .spawn(move || {
                        while active.load(Ordering::Acquire) {
                            engine.increment_epoch();
                            thread::sleep(Duration::from_millis(2));
                        }
                    });
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(sender: mpsc::Sender<WasmFiberStatus>) -> Self {
        Self {
            sender,
            state: Arc::new(AtomicU8::new(EXECUTION_RUNNING)),
            engine: None,
            active: Arc::new(AtomicBool::new(false)),
            ticker_started: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub(crate) struct WasmFiber {
    rt: tokio::runtime::Runtime,
    awaiter: mpsc::Receiver<WasmFiberStatus>,
    handle: Option<JoinHandle<Result<()>>>,
    canceler: WasmFiberCanceler,
    cancel_state: Arc<AtomicU8>,
}

impl WasmFiber {
    pub fn new(
        mut store: Store<SyncWasmStates>,
        instance_pre: InstancePre<SyncWasmStates>,
    ) -> Result<Self> {
        // Timers are needed for clocks to work properly in WASI.
        let rt = Builder::new_current_thread()
            .enable_time()
            .enable_io()
            .build()
            .unwrap();
        let (sender, awaiter) = mpsc::channel(1);
        let cancel_state = Arc::new(AtomicU8::new(EXECUTION_RUNNING));
        let active = Arc::new(AtomicBool::new(true));
        let active_guard = ExecutionActive {
            state: Arc::clone(&cancel_state),
            active: Arc::clone(&active),
        };
        let engine = store.engine().clone();
        configure_epoch_deadline(&mut store, Some(Arc::clone(&cancel_state)));
        let canceler = WasmFiberCanceler {
            sender: sender.clone(),
            state: Arc::clone(&cancel_state),
            engine: Some(engine),
            active,
            ticker_started: Arc::new(AtomicBool::new(false)),
        };
        let finalizer = sender.clone();
        store.data().state.lock().unwrap().yielder = Some(sender);
        let command_pre = CommandPre::new(instance_pre)?;

        let handle = rt.spawn(async move {
            let _active_guard = active_guard;
            let started = std::time::Instant::now();
            let command_result = command_pre.instantiate_async(&mut store).await;
            metrics::observe_instantiation(started.elapsed());
            let command = command_result?;

            let fuel_before = store.get_fuel().ok();
            let started = std::time::Instant::now();
            let run_result = command.wasi_cli_run().call_run(&mut store).await;
            metrics::observe_guest_execution(started.elapsed());
            metrics::observe_fuel(fuel_before, store.get_fuel().ok());
            handle_command_result(run_result)?;
            finalizer
                .send(WasmFiberStatus::Finished)
                .await
                .map_err(|e| anyhow!("failed to signal fiber completion: {e}"))?;
            Ok(())
        });
        Ok(WasmFiber {
            rt,
            awaiter,
            handle: Some(handle),
            canceler,
            cancel_state,
        })
    }

    pub(crate) fn canceler(&self) -> WasmFiberCanceler {
        self.canceler.clone()
    }

    fn cancellation_requested(&self) -> bool {
        self.cancel_state.load(Ordering::Acquire) == EXECUTION_CANCELED
    }

    fn cancel_task(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }

    fn resume(&mut self) -> Result<WasmFiberStatus> {
        let handle = self
            .handle
            .as_mut()
            .ok_or_else(|| anyhow!("fiber task is no longer available"))?;
        let status = self.rt.block_on(async {
            let (status, task_completed) = poll_fn(|cx| {
                if let Poll::Ready(result) = Pin::new(&mut *handle).poll(cx) {
                    return Poll::Ready(match result {
                        Ok(result) => result.map(|()| (WasmFiberStatus::Finished, true)),
                        Err(err) => Err(err.into()),
                    });
                }

                self.awaiter
                    .poll_recv(cx)
                    .map(|status| Ok((status.unwrap_or(WasmFiberStatus::Finished), false)))
            })
            .await?;
            if matches!(status, WasmFiberStatus::Finished) && !task_completed {
                (&mut *handle).await??;
            }
            Ok::<_, anyhow::Error>(status)
        })?;
        if matches!(status, WasmFiberStatus::Finished) {
            self.handle.take();
        }
        Ok(status)
    }

    fn resume_on_tx(self) -> Result<(WasmFiber, WasmFiberStatus)> {
        let (tx, rx) = oneshot::channel();
        let (ret_tx, ret_rx) = oneshot::channel();
        let (status_tx, status_rx) = oneshot::channel();
        let _ = tx.send(self);
        let cb = move || {
            if let Ok(mut fiber) = rx.blocking_recv() {
                let _sigstack_guard = SigAltStackRestoreGuard::capture();
                let status = fiber.resume();
                let _ = ret_tx.send(fiber);
                let _ = status_tx.send(status);
            }
        };

        tx_push(Box::new(TxCtx { cb: Box::new(cb) }))?;

        let fiber = ret_rx
            .blocking_recv()
            .map_err(|_| anyhow!("fiber cancelled during resume"))?;
        let status = status_rx
            .blocking_recv()
            .map_err(|_| anyhow!("fiber cancelled during resume"))??;
        Ok((fiber, status))
    }

    pub(crate) fn block_on(self) -> Result<()> {
        self.block_on_status().map(|_| ())
    }

    pub(crate) fn block_on_status(mut self) -> Result<WasmFiberStatus> {
        loop {
            if self.cancellation_requested() {
                self.cancel_task();
                return Ok(WasmFiberStatus::Canceled);
            }
            let status = match self.resume() {
                Ok(status) => status,
                Err(err) if err.chain().any(|cause| cause.is::<EpochCancellation>()) => {
                    return Ok(WasmFiberStatus::Canceled);
                }
                Err(_) if self.cancellation_requested() => {
                    return Ok(WasmFiberStatus::Canceled);
                }
                Err(err) => return Err(err),
            };
            if self.cancellation_requested() {
                self.cancel_task();
                return Ok(WasmFiberStatus::Canceled);
            }
            match status {
                WasmFiberStatus::Suspended => {
                    let (fiber, status) = self.resume_on_tx()?;
                    self = fiber;
                    if self.cancellation_requested() {
                        self.cancel_task();
                        return Ok(WasmFiberStatus::Canceled);
                    }
                    match status {
                        WasmFiberStatus::Suspended => {
                            continue;
                        }
                        WasmFiberStatus::Finished => {
                            return Ok(WasmFiberStatus::Finished);
                        }
                        WasmFiberStatus::Canceled => {
                            self.cancel_task();
                            return Ok(WasmFiberStatus::Canceled);
                        }
                    }
                }
                WasmFiberStatus::Finished => {
                    return Ok(WasmFiberStatus::Finished);
                }
                WasmFiberStatus::Canceled => {
                    self.cancel_task();
                    return Ok(WasmFiberStatus::Canceled);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_and_completion_have_one_winner() {
        use std::sync::Barrier;

        for _ in 0..100 {
            let (sender, _receiver) = mpsc::channel(1);
            let canceler = WasmFiberCanceler::for_test(sender);
            let state = Arc::clone(&canceler.state);
            let active = Arc::new(AtomicBool::new(true));
            let guard = ExecutionActive {
                state: Arc::clone(&state),
                active: Arc::clone(&active),
            };
            let barrier = Arc::new(Barrier::new(3));
            let cancel_barrier = Arc::clone(&barrier);
            let cancel_thread = thread::spawn(move || {
                cancel_barrier.wait();
                canceler.request();
            });
            let finish_barrier = Arc::clone(&barrier);
            let finish_thread = thread::spawn(move || {
                finish_barrier.wait();
                drop(guard);
            });
            barrier.wait();
            cancel_thread.join().unwrap();
            finish_thread.join().unwrap();
            assert!(matches!(
                state.load(Ordering::Acquire),
                EXECUTION_CANCELED | EXECUTION_COMPLETED
            ));
            assert!(!active.load(Ordering::Acquire));
        }
    }

    #[test]
    fn cancellation_latch_survives_a_full_status_channel() {
        let (sender, mut receiver) = mpsc::channel(1);
        sender.blocking_send(WasmFiberStatus::Suspended).unwrap();
        let state = Arc::new(AtomicU8::new(EXECUTION_RUNNING));
        let canceler = WasmFiberCanceler {
            sender,
            state: Arc::clone(&state),
            engine: None,
            active: Arc::new(AtomicBool::new(false)),
            ticker_started: Arc::new(AtomicBool::new(false)),
        };

        canceler.request();

        assert_eq!(state.load(Ordering::Acquire), EXECUTION_CANCELED);
        assert!(matches!(
            receiver.blocking_recv(),
            Some(WasmFiberStatus::Suspended)
        ));
    }
}
