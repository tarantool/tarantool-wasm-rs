use crate::bindings::error::{handle_tarantool_error, map_tarantool_error};
use crate::bindings::helpers::{debug_assert_tx_thread, illegal_params};
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
        self.executor()
            .execute(|| {
                debug_assert_tx_thread();
                unsafe { ffi::tnt::box_txn_id() }
            })
            .await
    }

    async fn txn_isolation(&mut self) -> TxnIsolationLevel {
        self.executor()
            .execute(|| {
                debug_assert_tx_thread();
                TxnIsolationLevel::from(unsafe { ffi::tnt::box_txn_isolation() })
            })
            .await
    }

    async fn txn(&mut self) -> bool {
        self.executor()
            .execute(|| {
                debug_assert_tx_thread();
                transaction::is_in_transaction()
            })
            .await
    }

    async fn begin(&mut self) -> Result<(), BoxError> {
        if !self.can_reschedule_to_tx() {
            return Err(illegal_params(
                "transactions require a reschedulable wasm.run execution",
            ));
        }
        self.yield_now().await;
        self.begin_tx();
        debug_assert_tx_thread();
        transaction::begin().map_err(|e| map_tarantool_error(e.into()))
    }

    async fn commit(&mut self) -> Result<(), BoxError> {
        self.executor()
            .execute(|| {
                debug_assert_tx_thread();
                transaction::commit().map_err(|e| map_tarantool_error(e.into()))
            })
            .await?;

        self.yield_now().await;
        self.end_tx();

        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), BoxError> {
        self.executor()
            .execute(|| {
                debug_assert_tx_thread();
                transaction::rollback().map_err(|e| map_tarantool_error(e.into()))
            })
            .await?;

        self.yield_now().await;
        self.end_tx();

        Ok(())
    }

    async fn txn_set_isolation(&mut self, isolation: TxnIsolationLevel) -> Result<(), BoxError> {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let rc = unsafe { ffi::tnt::box_txn_set_isolation(isolation as u32) };
                match rc {
                    0 => Ok(()),
                    _ => Err(handle_tarantool_error()),
                }
            })
            .await
    }

    async fn txn_make_sync(&mut self) {
        self.executor()
            .execute(|| {
                debug_assert_tx_thread();
                unsafe { ffi::tnt::box_txn_make_sync() }
            })
            .await
    }
}
