//! Execution helpers bridging the async Wasmtime runtime with Tarantool's
//! cooperative multitasking environment.

use std::time::Instant;

use crate::runtime::{
    metrics,
    tx::{tx_push, TxCtx},
};
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
        let queued_at = Instant::now();

        let closure = move || {
            metrics::observe_tx_wait(queued_at.elapsed());
            let res = task();
            let _ = tx.send(res);
        };

        tx_push(Box::new(TxCtx {
            cb: Box::new(closure),
        }))
        .expect("TX submission rejected while a WASM worker is active");

        // Every active worker owns a lifecycle guard. During Stopping its
        // TxCtx is moved to the shutdown queue and executed on TX before the
        // guard can be released, so shutdown cannot drop the sender.
        rx.await
            .expect("TX callback dropped despite an active worker lifecycle guard")
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
