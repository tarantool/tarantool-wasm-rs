use tokio::sync::mpsc;
use tokio::sync::oneshot;
use wasmtime::Store;
use wasmtime::component::{Component, Linker};
use crate::tx::{tx_push, tx_flush, TxCtx};
use std::sync::Arc;
use tokio::runtime::Builder;
use wasmtime_wasi::bindings::Command;
use crate::tarantool_wasm::SyncWasmStates;
use anyhow::Result;
use crate::executor::AdaptiveExecutor;
use std::sync::Mutex;

pub(crate) enum WasmFiberStatus {
    Suspended,
    Finished,
}

pub(crate) struct WasmFiber {
    rt: tokio::runtime::Runtime,
    awaiter: mpsc::Receiver<WasmFiberStatus>,
}

impl WasmFiber {
    pub fn new(mut store: Store<SyncWasmStates>, linker: Arc<Linker<SyncWasmStates>>, component: Component) -> Self {
        // Timers are needed for clocks to work properly in WASI.
        let rt = Builder::new_current_thread().enable_time().build().unwrap();
        let (yielder, awaiter) = mpsc::channel(1);
        let finalizer = yielder.clone();
        {
            let mut state = store.data().0.lock().unwrap();
            state.yielder = Some(yielder);
            let executor = Arc::new(Mutex::new(AdaptiveExecutor::new()));
            state.executor = Some(executor);
        }

        rt.spawn(async move {
            let command = Command::instantiate_async(&mut store, &component, &linker).await?;
            let result = command.wasi_cli_run().call_run(&mut store).await;
            finalizer.send(WasmFiberStatus::Finished).await?;
            Ok::<Result<(), ()>, wasmtime::Error>(result?)
        });
        WasmFiber{rt, awaiter}
    }

    fn resume(&mut self) -> WasmFiberStatus {
        self.rt.block_on(async {
            self.awaiter.recv().await.unwrap()
        })
    }

    fn resume_on_tx(self) -> WasmFiber {
        let (tx, rx) = oneshot::channel();
        let (ret_tx, ret_rx) = oneshot::channel();
        tx.send(self);
        let cb = move || {
            let mut fiber = rx.blocking_recv().unwrap();
            fiber.resume();
            ret_tx.send(fiber);
        };

        tx_push(
            Box::new(TxCtx{cb: Box::new(cb)})
        );
        tx_flush();

        ret_rx.blocking_recv().unwrap()
    }

    pub(crate) fn block_on(mut self) -> Result<()>{
        loop {
            match self.resume() {
                WasmFiberStatus::Suspended => {
                    self = self.resume_on_tx();
                },
                WasmFiberStatus::Finished => {
                    return Ok(());
                },
            }
        }
    }
}
