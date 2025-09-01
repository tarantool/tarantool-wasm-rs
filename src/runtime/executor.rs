//! Execution helpers bridging the async Wasmtime runtime with Tarantool's
//! cooperative multitasking environment.

use crate::runtime::tx::{tx_flush, tx_push, TxCtx};
use tokio::sync::oneshot;
use wasmtime_wasi::async_trait;

/// Mode of transactional execution inside Tarantool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxMode {
    /// No transaction is active.
    None,
    /// Transaction has been started.
    Started,
}

/// An abstraction over different executor strategies.
#[async_trait]
pub trait ExecutorImpl: Send + Sync {
    /// Execute the provided task and return its result.
    async fn execute<F, R>(&self, task: F) -> R
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static;
}

/// Executor that simply runs tasks on the current thread.
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

/// Executor that schedules tasks to be executed inside Tarantool's transaction
/// thread.
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

        tx_push(Box::new(TxCtx {
            cb: Box::new(closure),
        }));
        tx_flush();

        rx.await
            .unwrap_or_else(|_| panic!("TX thread failed to respond"))
    }
}

/// Public enum selecting between available executor implementations.
#[derive(Debug, Clone, Copy)]
pub enum Executor {
    /// Execute tasks directly.
    Direct,
    /// Push tasks to the transaction thread.
    TxPush,
}

impl Executor {
    /// Execute a closure using the selected strategy.
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
