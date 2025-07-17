use crate::tarantool_wasm::SyncWasmStates;
use crate::tx::{tx_flush, tx_push, TxCtx};
use anyhow::Result;
use std::sync::Arc;
use tokio::runtime::Builder;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use wasmtime::component::{Component, Linker};
use wasmtime::Store;
use wasmtime_wasi::p2::bindings::Command;

pub(crate) enum WasmFiberStatus {
    Suspended,
    Finished,
}

pub(crate) struct WasmFiber {
    rt: tokio::runtime::Runtime,
    awaiter: mpsc::Receiver<WasmFiberStatus>,
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

        rt.spawn(async move {
            let command = Command::instantiate_async(&mut store, &component, &linker).await?;
            let result = command.wasi_cli_run().call_run(&mut store).await;
            finalizer.send(WasmFiberStatus::Finished).await?;
            Ok::<Result<(), ()>, wasmtime::Error>(result?)
        });
        WasmFiber { rt, awaiter }
    }

    fn resume(&mut self) -> WasmFiberStatus {
        self.rt
            .block_on(async { self.awaiter.recv().await.unwrap() })
    }

    fn resume_on_tx(self) -> WasmFiber {
        let (tx, rx) = oneshot::channel();
        let (ret_tx, ret_rx) = oneshot::channel();
        let _ = tx.send(self);
        let cb = move || {
            let mut fiber = rx.blocking_recv().unwrap();
            fiber.resume();
            let _ = ret_tx.send(fiber);
        };

        tx_push(Box::new(TxCtx { cb: Box::new(cb) }));
        tx_flush();

        ret_rx.blocking_recv().unwrap()
    }

    pub(crate) fn block_on(mut self) -> Result<()> {
        loop {
            match self.resume() {
                WasmFiberStatus::Suspended => {
                    self = self.resume_on_tx();
                }
                WasmFiberStatus::Finished => {
                    return Ok(());
                }
            }
        }
    }
}
