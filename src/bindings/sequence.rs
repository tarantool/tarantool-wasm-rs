use crate::bindings::docs::adder::sequence;
use crate::bindings::docs::adder::types::{BoxError, Sequence};
use crate::tarantool_wasm::SyncWasmStates;
use crate::bindings::error::handle_tarantool_error;
use crate::ffi;
use crate::tx::{TxCtx, tx_flush, tx_push};

use wasmtime_wasi::async_trait;
use tokio::sync::oneshot;
use crate::bindings::ttbox::{begin_transaction, commit_transaction};

#[async_trait]
impl sequence::Host for SyncWasmStates {

    async fn next(&mut self, seq: Sequence) -> Result<i64, BoxError> {
        let (tx, rx) = oneshot::channel();
        
        let task = Box::new(TxCtx {
            cb: Box::new(move || {

                unsafe {
                    if let Err(err) = begin_transaction() {
                        let _ = tx.send(Err(err));
                        return;
                    }
                }

                let mut res: i64 = 0;
                let rc = unsafe { ffi::r#box::box_sequence_next(seq.id as u32, &mut res) };

                if rc != 0 {
                    unsafe { ffi::tnt::box_txn_rollback(); }
                    let err = unsafe { handle_tarantool_error() };
                    let _ = tx.send(Err(err));
                    return;
                }

                unsafe {
                    if let Err(err) = commit_transaction() {
                        ffi::tnt::box_txn_rollback();
                        let _ = tx.send(Err(err));
                        return;
                    }
                }

                let _ = tx.send(Ok(res));

            }),
        });
        tx_push(task);
        tx_flush();

        rx.await.unwrap_or_else(|_| Err(BoxError::new("Failed to receive response from TX thread", "internal error", 500)))
    }

    async fn current(&mut self, seq: Sequence) -> Result<i64, BoxError> {
        let mut res: i64 = 0;
        let rc = unsafe { ffi::r#box::box_sequence_current(seq.id as u32, &mut res) };
        if rc != 0 {
            let err = unsafe { handle_tarantool_error() };
            println!("Error sequence.current: {}", err.message);
            Err(err)
        } else {
            Ok(res)
        }
    }

    async fn set(&mut self, seq: Sequence, value: i64) -> Result<(), BoxError> {
        let (tx, rx) = oneshot::channel();
        
        let task = Box::new(TxCtx {
            cb: Box::new(move || {

                unsafe {
                    if let Err(err) = begin_transaction() {
                        let _ = tx.send(Err(err));
                        return;
                    }
                }

                let rc = unsafe { ffi::r#box::box_sequence_set(seq.id as u32, value) };

                if rc != 0 {
                    unsafe { ffi::tnt::box_txn_rollback(); }
                    let err = unsafe { handle_tarantool_error() };
                    let _ = tx.send(Err(err));
                    return;
                }

                unsafe {
                    if let Err(err) = commit_transaction() {
                        ffi::tnt::box_txn_rollback();
                        let _ = tx.send(Err(err));
                        return;
                    }
                }

                let _ = tx.send(Ok(()));

            }),
        });
        tx_push(task);
        tx_flush();

        rx.await.unwrap_or_else(|_| Err(BoxError::new("Failed to receive response from TX thread", "internal error", 500)))
    }

    async fn reset(&mut self, seq: Sequence) -> Result<(), BoxError> {
        let (tx, rx) = oneshot::channel();
        
        let task = Box::new(TxCtx {
            cb: Box::new(move || {

                unsafe {
                    if let Err(err) = begin_transaction() {
                        let _ = tx.send(Err(err));
                        return;
                    }
                }

                let rc = unsafe { ffi::r#box::box_sequence_reset(seq.id as u32) };

                if rc != 0 {
                    unsafe { ffi::tnt::box_txn_rollback(); }
                    let err = unsafe { handle_tarantool_error() };
                    let _ = tx.send(Err(err));
                    return;
                }

                unsafe {
                    if let Err(err) = commit_transaction() {
                        ffi::tnt::box_txn_rollback();
                        let _ = tx.send(Err(err));
                        return;
                    }
                }

                let _ = tx.send(Ok(()));

            }),
        });
        tx_push(task);
        tx_flush();

        rx.await.unwrap_or_else(|_| Err(BoxError::new("Failed to receive response from TX thread", "internal error", 500)))
    }
}