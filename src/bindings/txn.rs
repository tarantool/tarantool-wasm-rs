use crate::bindings::error::{handle_tarantool_error, map_tarantool_error};
use crate::bindings::helpers::{debug_assert_tx_thread, illegal_params, unknown_error};
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
        if let Err(err) = self.begin_tx() {
            return Err(unknown_error(format!(
                "failed to start transaction fiber: {err:#}"
            )));
        }
        let (res, active, id) = self
            .executor()
            .execute(|| {
                debug_assert_tx_thread();
                let res = tnt_transaction::begin().map_err(|e| map_tarantool_error(e.into()));
                let id = unsafe { ffi::tnt::box_txn_id() };
                (
                    res,
                    tnt_transaction::is_in_transaction(),
                    (id >= 0).then_some(id as u64),
                )
            })
            .await;
        if !active {
            // If BEGIN failed without an active transaction, release the
            // owner. A duplicate BEGIN leaves the original owner active.
            self.end_tx();
        } else if res.is_ok() {
            if let Some(id) = id {
                self.set_tx_id(id);
            } else {
                self.end_tx();
                return Err(unknown_error(
                    "BEGIN succeeded without a host transaction id",
                ));
            }
        }
        res
    }

    async fn commit(&mut self) -> Result<(), BoxError> {
        let Some((fiber, expected_id)) = self.transaction_owner() else {
            self.end_tx();
            return Err(unknown_error("COMMIT without an active host transaction"));
        };
        let res = fiber
            .call(move || {
                debug_assert_tx_thread();
                let actual_id = unsafe { ffi::tnt::box_txn_id() };
                if actual_id < 0 || actual_id as u64 != expected_id {
                    return Err(unknown_error("host transaction was lost before COMMIT"));
                }
                tnt_transaction::commit().map_err(|e| map_tarantool_error(e.into()))
            })
            .await
            .map_err(|err| unknown_error(format!("host transaction unavailable: {err:#}")))
            .and_then(|res| res);
        // Tear the fiber down unconditionally: a failed commit must not strand
        // the executor on `TxFiber`, and the fiber rolls back anything the guest
        // left open.
        self.end_tx();
        res
    }

    async fn rollback(&mut self) -> Result<(), BoxError> {
        let Some((fiber, expected_id)) = self.transaction_owner() else {
            self.end_tx();
            return Err(unknown_error("ROLLBACK without an active host transaction"));
        };
        let res = fiber
            .call(move || {
                debug_assert_tx_thread();
                let actual_id = unsafe { ffi::tnt::box_txn_id() };
                if actual_id < 0 || actual_id as u64 != expected_id {
                    return Err(unknown_error("host transaction was lost before ROLLBACK"));
                }
                tnt_transaction::rollback().map_err(|e| map_tarantool_error(e.into()))
            })
            .await
            .map_err(|err| unknown_error(format!("host transaction unavailable: {err:#}")))
            .and_then(|res| res);
        self.end_tx();
        res
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
