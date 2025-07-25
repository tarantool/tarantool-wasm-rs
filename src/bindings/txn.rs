use crate::bindings::error::handle_tarantool_error;
use crate::bindings::tarantool::tarantool::txn;
use crate::bindings::tarantool::tarantool::types::{BoxError, TxnIsolationLevel};
use crate::ffi;
use crate::runtime::tarantool_wasm::SyncWasmStates;

use tarantool::transaction;

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

impl txn::Host for SyncWasmStates {
    async fn txn_id(&mut self) -> i64 {
        unsafe { ffi::tnt::box_txn_id() }
    }

    async fn txn_isolation(&mut self) -> TxnIsolationLevel {
        let level = unsafe { ffi::tnt::box_txn_isolation() };
        TxnIsolationLevel::from(level)
    }

    async fn txn(&mut self) -> bool {
        transaction::is_in_transaction()
    }

    async fn begin(&mut self) -> Result<(), BoxError> {
        self.yield_now().await;
        self.begin_tx();

        transaction::begin().map_err(|e| BoxError::new(e.to_string(), "TransactionError", 1))
    }

    async fn commit(&mut self) -> Result<(), BoxError> {
        transaction::commit().map_err(|e| BoxError::new(e.to_string(), "TransactionError", 2))?;

        self.yield_now().await;
        self.end_tx();

        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), BoxError> {
        transaction::rollback().map_err(|e| BoxError::new(e.to_string(), "TransactionError", 2))?;

        self.yield_now().await;
        self.end_tx();

        Ok(())
    }

    async fn txn_set_isolation(&mut self, isolation: TxnIsolationLevel) -> Result<(), BoxError> {
        let rc = unsafe { ffi::tnt::box_txn_set_isolation(isolation as u32) };
        match rc {
            0 => Ok(()),
            _ => Err(handle_tarantool_error()),
        }
    }

    async fn txn_make_sync(&mut self) {
        unsafe { ffi::tnt::box_txn_make_sync() }
    }
}
