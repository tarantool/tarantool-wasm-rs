use std::{
    future::{poll_fn, Future},
    mem,
    pin::Pin,
    ptr,
    task::Poll,
};

use anyhow::{anyhow, Result};
use tokio::{
    runtime::Builder,
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use wasmtime::{component::InstancePre, Store};
use wasmtime_wasi::p2::bindings::CommandPre;
use wasmtime_wasi::I32Exit;

use crate::runtime::{
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

pub(crate) enum WasmFiberStatus {
    Suspended,
    Finished,
    Canceled,
}

pub(crate) struct WasmFiber {
    rt: tokio::runtime::Runtime,
    awaiter: mpsc::Receiver<WasmFiberStatus>,
    handle: Option<JoinHandle<Result<()>>>,
    canceler: mpsc::Sender<WasmFiberStatus>,
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
        let canceler = sender.clone();
        let finalizer = sender.clone();
        store.data().state.lock().unwrap().yielder = Some(sender);
        let command_pre = CommandPre::new(instance_pre)?;

        let handle = rt.spawn(async move {
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
        })
    }

    pub(crate) fn canceler(&self) -> mpsc::Sender<WasmFiberStatus> {
        self.canceler.clone()
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

    pub(crate) fn block_on(mut self) -> Result<()> {
        loop {
            match self.resume()? {
                WasmFiberStatus::Suspended => {
                    let (fiber, status) = self.resume_on_tx()?;
                    self = fiber;
                    match status {
                        WasmFiberStatus::Suspended => {
                            continue;
                        }
                        WasmFiberStatus::Finished => {
                            return Ok(());
                        }
                        WasmFiberStatus::Canceled => {
                            self.cancel_task();
                            return Ok(());
                        }
                    }
                }
                WasmFiberStatus::Finished => {
                    return Ok(());
                }
                WasmFiberStatus::Canceled => {
                    self.cancel_task();
                    return Ok(());
                }
            }
        }
    }
}
