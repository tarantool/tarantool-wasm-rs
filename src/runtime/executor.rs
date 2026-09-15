//! Execution helpers bridging the async Wasmtime runtime with Tarantool's
//! cooperative multitasking environment.

use std::sync::Arc;
use std::time::Instant;

use crate::runtime::{
    metrics,
    tx::{tx_push, TxCtx},
    tx_fiber::{Job, TxFiber},
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

/// Executor that runs tasks on the dedicated fiber owning the current
/// transaction (see [`crate::runtime::tx_fiber`]).
///
/// Unlike [`TxPushExecutor`], every task is dispatched to the *same* fiber, so
/// fiber-local `box` transaction state is preserved across calls without
/// keeping the guest itself resident on the TX thread.
#[derive(Debug)]
pub struct TxFiberExecutor {
    fiber: Arc<TxFiber>,
}

#[async_trait]
impl ExecutorImpl for TxFiberExecutor {
    async fn execute<F, R>(&self, task: F) -> R
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let (reply_tx, reply_rx) = oneshot::channel();
        let job: Job = Box::new(move || {
            let _ = reply_tx.send(task());
        });

        self.fiber
            .submit(job)
            .unwrap_or_else(|_| panic!("transaction fiber is no longer available"));

        reply_rx
            .await
            .expect("transaction fiber dropped a job without replying")
    }
}

/// Public enum selecting between available executor implementations.
#[derive(Debug, Clone)]
pub enum Executor {
    /// Push each task to the transaction thread independently. Used whenever no
    /// Tarantool transaction is open.
    TxPush,
    /// Run each task on the dedicated fiber that owns the open transaction.
    TxFiber(Arc<TxFiber>),
}

impl Executor {
    /// Execute a closure using the selected strategy.
    pub async fn execute<F, R>(&self, task: F) -> R
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        match self {
            Executor::TxPush => TxPushExecutor.execute(task).await,
            Executor::TxFiber(fiber) => {
                TxFiberExecutor {
                    fiber: Arc::clone(fiber),
                }
                .execute(task)
                .await
            }
        }
    }
}
