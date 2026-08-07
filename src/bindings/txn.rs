use crate::bindings::error::{handle_tarantool_error, map_tarantool_error};
use crate::bindings::helpers::{debug_assert_tx_thread, illegal_params};
use crate::bindings::tarantool::tarantool::transaction;
use crate::bindings::tarantool::tarantool::types::{BoxError, TransactionIsolation};
use crate::ffi;
use crate::runtime::tarantool_wasm::SyncWasmStates;

use tarantool::transaction as tnt_transaction;

fn isolation_from_i32(value: i32) -> Option<TransactionIsolation> {
    match value {
        0 => Some(TransactionIsolation::Default),
        1 => Some(TransactionIsolation::BestEffort),
        2 => Some(TransactionIsolation::ReadCommitted),
        3 => Some(TransactionIsolation::ReadConfirmed),
        4 => Some(TransactionIsolation::Linearizable),
        _ => None,
    }
}

impl transaction::Host for SyncWasmStates {
    async fn id(&mut self) -> Option<u64> {
        self.executor()
            .execute(|| {
                debug_assert_tx_thread();
                let id = unsafe { ffi::tnt::box_txn_id() };
                (id >= 0).then_some(id as u64)
            })
            .await
    }

    async fn isolation(&mut self) -> Option<TransactionIsolation> {
        self.executor()
            .execute(|| {
                debug_assert_tx_thread();
                isolation_from_i32(unsafe { ffi::tnt::box_txn_isolation() })
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
        tnt_transaction::begin().map_err(|e| map_tarantool_error(e.into()))
    }

    async fn commit(&mut self) -> Result<(), BoxError> {
        self.executor()
            .execute(|| {
                debug_assert_tx_thread();
                tnt_transaction::commit().map_err(|e| map_tarantool_error(e.into()))
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
                tnt_transaction::rollback().map_err(|e| map_tarantool_error(e.into()))
            })
            .await?;

        self.yield_now().await;
        self.end_tx();

        Ok(())
    }

    async fn set_isolation(&mut self, isolation: TransactionIsolation) -> Result<(), BoxError> {
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

    async fn make_synchronous(&mut self) {
        self.executor()
            .execute(|| {
                debug_assert_tx_thread();
                unsafe { ffi::tnt::box_txn_make_sync() }
            })
            .await
    }
}
