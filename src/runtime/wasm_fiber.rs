use std::sync::Arc;

use anyhow::{anyhow, Result};
use tokio::{
    runtime::Builder,
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use wasmtime::{
    component::{Component, Linker},
    Store,
};
use wasmtime_wasi::p2::bindings::Command;

use crate::runtime::{
    tarantool_wasm::SyncWasmStates,
    tx::{tx_flush, tx_push, TxCtx},
};

pub(crate) enum WasmFiberStatus {
    Suspended,
    Finished,
}

pub(crate) struct WasmFiber {
    rt: tokio::runtime::Runtime,
    awaiter: mpsc::Receiver<WasmFiberStatus>,
    handle: Option<JoinHandle<Result<()>>>,
}

impl WasmFiber {
    pub fn new(
        mut store: Store<SyncWasmStates>,
        linker: Arc<Linker<SyncWasmStates>>,
        component: Component,
    ) -> Self {
        // Timers are needed for clocks to work properly in WASI.
        let rt = Builder::new_current_thread()
            .enable_time()
            .enable_io()
            .build()
            .unwrap();
        let (yielder, awaiter) = mpsc::channel(1);
        let finalizer = yielder.clone();
        store.data().0.lock().unwrap().yielder = Some(yielder);

        let handle = rt.spawn(async move {
            let command = Command::instantiate_async(&mut store, &component, &linker).await?;
            let _ = command.wasi_cli_run().call_run(&mut store).await?;
            finalizer.send(WasmFiberStatus::Finished).await
                .map_err(|e| anyhow!("failed to signal fiber completion: {e}"))?;
            Ok(())
        });
        WasmFiber {
            rt,
            awaiter,
            handle: Some(handle),
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

    fn resume_on_tx(self) -> Result<WasmFiber> {
        let (tx, rx) = oneshot::channel();
        let (ret_tx, ret_rx) = oneshot::channel();
        let _ = tx.send(self);
        let cb = move || {
            if let Ok(mut fiber) = rx.blocking_recv() {
                let _ = fiber.resume();
                let _ = ret_tx.send(fiber);
            }
        };

        tx_push(Box::new(TxCtx { cb: Box::new(cb) }));
        tx_flush();

        ret_rx
            .blocking_recv()
            .map_err(|_| anyhow!("fiber cancelled during resume"))
    }

    pub(crate) fn block_on(mut self) -> Result<()> {
        loop {
            match self.resume()? {
                WasmFiberStatus::Suspended => {
                    self = self.resume_on_tx()?;
                }
                WasmFiberStatus::Finished => {
                    return Ok(());
                }
            }
        }
    }
}
