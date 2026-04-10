use std::{future::Future, mem, ptr};

use anyhow::{anyhow, Result};
use tokio::{
    runtime::Builder,
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use wasmtime::Store;

use crate::runtime::{
    tarantool_wasm::SyncWasmStates,
    tx::{tx_flush, tx_push, TxCtx},
};

struct SigAltStackRestoreGuard {
    previous: Option<libc::stack_t>,
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
    pub fn new<F, Fut>(store: Store<SyncWasmStates>, runner: F) -> Self
    where
        F: FnOnce(Store<SyncWasmStates>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
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

        let handle = rt.spawn(async move {
            runner(store).await?;
            finalizer
                .send(WasmFiberStatus::Finished)
                .await
                .map_err(|e| anyhow!("failed to signal fiber completion: {e}"))?;
            Ok(())
        });
        WasmFiber {
            rt,
            awaiter,
            handle: Some(handle),
            canceler,
        }
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
        self.rt.block_on(async {
            match self.awaiter.recv().await {
                Some(status) => Ok(status),
                None => {
                    if let Some(handle) = self.handle.take() {
                        handle.await??;
                    }
                    Ok(WasmFiberStatus::Finished)
                }
            }
        })
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

        tx_push(Box::new(TxCtx { cb: Box::new(cb) }));
        tx_flush();

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
