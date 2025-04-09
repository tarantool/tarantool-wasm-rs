use wasmtime_wasi::async_trait;
use crate::tx::{tx_push, tx_flush, TxCtx};
use tokio::sync::oneshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxMode {
    None,
    Started,
}

#[async_trait]
pub trait ExecutorImpl: Send + Sync {
    async fn execute<F, R>(&self, task: F) -> R
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static;
}

#[derive(Debug)]
pub struct DirectExecutor;

#[async_trait]
impl ExecutorImpl for DirectExecutor {
    async fn execute<F, R>(&self, task: F) -> R
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        task()
    }
}

#[derive(Debug)]
pub struct TxPushExecutor;

#[async_trait]
impl ExecutorImpl for TxPushExecutor {
    async fn execute<F, R>(&self, task: F) -> R
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = oneshot::channel();

        let closure = move || {
            let res = task();
            let _ = tx.send(res);
        };

        tx_push(Box::new(TxCtx { cb: Box::new(closure) }));
        tx_flush();

        rx.await.unwrap_or_else(|_| panic!("TX thread failed to respond"))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Executor {
    Direct,
    TxPush,
}

impl Executor {
    pub async fn execute<F, R>(&self, task: F) -> R
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        match self {
            Executor::Direct => DirectExecutor.execute(task).await,
            Executor::TxPush => TxPushExecutor.execute(task).await,
        }
    }
}
