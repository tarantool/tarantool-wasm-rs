use crate::bindings::docs::adder::txn;
use crate::bindings::docs::adder::types::{BoxError, TxnIsolationLevel};
use crate::tarantool_wasm::SyncWasmStates;
use crate::{executor, ffi as ffi};
use crate::bindings::error::handle_tarantool_error;

use wasmtime_wasi::async_trait;

impl From<i32> for TxnIsolationLevel {

    fn from(value: i32) -> Self {
        match value {
            1 => TxnIsolationLevel::TxnIsolationBestEffort,
            2 => TxnIsolationLevel::TxnIsolationReadCommitted,
            3 => TxnIsolationLevel::TxnIsolationReadConfirmed,
            4 => TxnIsolationLevel::TxnIsolationLinearizable,
            _ => TxnIsolationLevel::TxnIsolationDefault,
        }
    }
}

#[async_trait]
impl txn::Host for SyncWasmStates {
    async fn txn_id(&mut self) -> i64 {
        unsafe { ffi::tnt::box_txn_id() }
    }

    async fn txn_isolation(&mut self) -> TxnIsolationLevel {
        let level = unsafe { ffi::tnt::box_txn_isolation() };
        TxnIsolationLevel::from(level)
    }

    async fn txn(&mut self) -> bool {
        unsafe { ffi::tnt::box_txn() }
    }

    async fn begin(&mut self) -> Result<(), BoxError> {
        self.yield_now().await;
        let rt = unsafe { ffi::tnt::box_txn_begin() };
        
        if let Some(exec) = self.executor() {
            let mut executor = exec.lock().unwrap();
            executor.begin_tx();
        } else {
            return Err(BoxError::new("?", "?", 999))
        }

        if rt != 0 {
            let err = unsafe { handle_tarantool_error() };
            Err(err)
        } else {
            Ok(())
        }
    }

    async fn commit(&mut self) -> Result<(), BoxError> {
        let rt = unsafe { ffi::tnt::box_txn_commit() };
        self.yield_now().await;

        if let Some(exec) = self.executor() {
            let mut executor = exec.lock().unwrap();
            executor.end_tx();
        } else {
            return Err(BoxError::new("?", "?", 999))
        }

        if rt != 0 {
            let err = unsafe { handle_tarantool_error() };
            Err(err)
        } else {
            Ok(())
        }
    }

    async fn rollback(&mut self) -> Result<(), BoxError> {
        let rt = unsafe { ffi::tnt::box_txn_rollback() };
        self.yield_now().await;

        if let Some(exec) = self.executor() {
            let mut executor = exec.lock().unwrap();
            executor.end_tx();
        } else {
            return Err(BoxError::new("?", "?", 999))
        }

        if rt != 0 {
            let err = unsafe { handle_tarantool_error() };
            Err(err)
        } else {
            Ok(())
        }
    }

    async fn txn_set_isolation(&mut self, isolation: TxnIsolationLevel) -> Result<(), BoxError> {
        let rc = unsafe { ffi::tnt::box_txn_set_isolation(isolation as u32) };
        match rc {
            0 => Ok(()),
            _ => Err(unsafe{ handle_tarantool_error() })
        }
    }

    async fn txn_make_sync(&mut self) {
        unsafe { ffi::tnt::box_txn_make_sync() }
    }
    
}