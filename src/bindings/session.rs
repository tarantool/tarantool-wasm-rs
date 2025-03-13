use crate::bindings::docs::adder::session;
use crate::tarantool_wasm::SyncWasmStates;
use crate::bindings::docs::adder::types::{BoxError, Session};
use crate::tx::{tx_push, tx_flush, TxCtx};

use wasmtime_wasi::async_trait;
use tokio::sync::oneshot;
use tarantool::error::TarantoolError;
use std::ffi::CStr;

extern "C" {
    fn box_txn() -> bool;
    fn box_txn_begin() -> i32;
    fn box_txn_commit() -> i32;
    fn box_txn_rollback() -> i32;

    pub fn box_session_id() -> u64;
    pub fn box_iproto_send(
        sid: u64,
        header: *const u8,
        header_end: *const u8,
        body: *const u8,
        body_end: *const u8,
    ) -> i32;

    fn box_error_last() -> *const TarantoolError;
    fn box_error_message(error: *const TarantoolError) -> *const i8;
}

#[async_trait]
impl session::Host for SyncWasmStates {

    async fn current(&mut self) -> Session {
        let session_id = unsafe { box_session_id() };
        return Session {
            id: session_id as i64,
        }
    }

    async fn iproto_send(&mut self, session: Session, header: Vec<u8>, body: Vec<u8>) -> Result<(), BoxError> {
        println!("RS | iproto_send() called for session_id: {}", session.id);

        if header.is_empty() {
            return Err(BoxError {
                message: "Header is empty".to_string(),
                type_: "validation error".to_string(),
                code: 1,
                payload: None,
                file: None,
                line: None,
            });
        }

        if body.is_empty() {
            return Err(BoxError {
                message: "Body is empty".to_string(),
                type_: "validation error".to_string(),
                code: 2,
                payload: None,
                file: None,
                line: None,
            });
        }

        let header_heap = header.clone();
        let body_heap = body.clone();
        let (tx, rx) = oneshot::channel();

        let task = Box::new(TxCtx {
            cb: Box::new(move || {
                println!(
                    "RS | iproto_send() header (MsgPack): {:?}, body (MsgPack): {:?}",
                    hex::encode(&header_heap),
                    hex::encode(&body_heap)
                );

                unsafe {
                    if box_txn_begin() != 0 {
                        let err_message = "Failed to start transaction".to_string();
                        println!("RS | Transaction begin failed: {}", err_message);
                        let _ = tx.send(Err(BoxError {
                            message: err_message,
                            type_: "Tarantool error".to_string(),
                            code: 1337,
                            payload: None,
                            file: None,
                            line: None,
                        }));
                        return;
                    }
                }

                let header_start = header_heap.as_ptr();
                let header_end = unsafe { header_start.add(header_heap.len()) };
                let body_start = body_heap.as_ptr();
                let body_end = unsafe { body_start.add(body_heap.len()) };

                let rc = unsafe {
                    box_iproto_send(session.id as u64, header_start, header_end, body_start, body_end)
                };

                if rc != 0 {
                    unsafe { box_txn_rollback() };

                    let err_ptr = unsafe { box_error_last() };
                    let err_message = if !err_ptr.is_null() {
                        unsafe {
                            CStr::from_ptr(box_error_message(err_ptr))
                                .to_string_lossy()
                                .into_owned()
                        }
                    } else {
                        "Unknown Tarantool error".to_string()
                    };

                    println!("RS | iproto_send() error: {}", err_message);
                    let _ = tx.send(Err(BoxError {
                        message: err_message,
                        type_: "tarantool error".to_string(),
                        code: 500,
                        payload: None,
                        file: None,
                        line: None,
                    }));
                    return;
                }

                unsafe {
                    if box_txn_commit() != 0 {
                        let err_message = "Failed to commit transaction".to_string();
                        println!("RS | Transaction commit failed: {}", err_message);
                        let _ = tx.send(Err(BoxError {
                            message: err_message,
                            type_: "Tarantool error".to_string(),
                            code: 1337,
                            payload: None,
                            file: None,
                            line: None,
                        }));
                        return;
                    }
                }

                println!("RS | iproto_send() successful for session_id: {}", session.id);
                let _ = tx.send(Ok(()));
            }),
        });

        tx_push(task);
        tx_flush();

        match rx.await {
            Ok(result) => result,
            Err(_) => Err(BoxError {
                message: "Failed to receive response from TX thread".to_string(),
                type_: "internal error".to_string(),
                code: 500,
                payload: None,
                file: None,
                line: None,
            }),
        }
    }

    async fn broadcast(&mut self, key: String, value: Vec<u8>) -> Result<(), BoxError> {
        Ok(())
    }

}

