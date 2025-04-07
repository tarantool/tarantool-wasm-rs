use wasmtime_wasi::async_trait;
use crate::tx::{tx_push, tx_flush, TxCtx};

#[derive(Clone, Copy, Debug)]
pub enum TxMode {
    None,
    Started,
}

#[async_trait]
pub trait Executor {
    async fn execute<F, R>(&self, func: F) -> R
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static;
}

#[derive(Clone)]
pub struct TxPushExecutor;

#[async_trait]
impl Executor for TxPushExecutor {
    async fn execute<F, R>(&self, func: F) -> R
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {

        let (tx, rx) = tokio::sync::oneshot::channel();
        let closure = move || {
            let res = func();
            let _ = tx.send(res);
        };

        tx_push(Box::new(TxCtx { cb: Box::new(closure) }));
        tx_flush();

        rx.await.unwrap_or_else(|_| {
            panic!("Failed to receive result from TX thread");
        })
    }
}


#[derive(Clone)]
pub struct InPlaceExecutor;

#[async_trait]
impl Executor for InPlaceExecutor {
    async fn execute<F, R>(&self, func: F) -> R
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        func()
    }
}

#[derive(Clone)]
pub struct AdaptiveExecutor {
    pub mode: TxMode,
    pub tx_push: TxPushExecutor,
    pub in_place: InPlaceExecutor,
}

impl AdaptiveExecutor {
    pub fn new() -> Self {
        Self {
            mode: TxMode::None,
            tx_push: TxPushExecutor,
            in_place: InPlaceExecutor,
        }
    }

    pub fn begin_tx(&mut self) {
        self.mode = TxMode::Started;
    }

    pub fn end_tx(&mut self) {
        self.mode = TxMode::None;
    }
}

#[async_trait]
impl Executor for AdaptiveExecutor {
    async fn execute<F, R>(&self, func: F) -> R
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        match self.mode {
            TxMode::None => self.tx_push.execute(func).await,
            TxMode::Started => self.in_place.execute(func).await,
        }
    }
}
