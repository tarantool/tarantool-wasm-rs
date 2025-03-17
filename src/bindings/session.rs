use crate::bindings::docs::adder::session;
use crate::tarantool_wasm::SyncWasmStates;
use crate::bindings::docs::adder::types::{BoxError, Session};
use crate::tx::{tx_push, tx_flush, TxCtx};
use crate::ffi as ffi;
use crate::bindings::ttbox::{begin_transaction, commit_transaction};
use crate::bindings::error::handle_tarantool_error;
use sha2::{Digest, Sha256};

use wasmtime_wasi::async_trait;
use tokio::sync::oneshot;

#[async_trait]
impl session::Host for SyncWasmStates {

    async fn current(&mut self) -> Session {
        let session_id = unsafe { ffi::session::box_session_id() };
        return Session {
            id: session_id as i64,
        }
    }

    async fn iproto_send(&mut self, session: Session, header: Vec<u8>, body: Vec<u8>) -> Result<(), BoxError> {
        println!("RS | iproto_send() called for session_id: {}", session.id);

        if header.is_empty() {
            return Err(BoxError::new("Header is empty", "validation error", 1));
        }
        if body.is_empty() {
            return Err(BoxError::new("Body is empty", "validation error", 1));
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
                    if let Err(err) = begin_transaction() {
                        let _ = tx.send(Err(err));
                        return;
                    }
                }

                let header_start = header_heap.as_ptr();
                let header_end = unsafe { header_start.add(header_heap.len()) };
                let body_start = body_heap.as_ptr();
                let body_end = unsafe { body_start.add(body_heap.len()) };

                let rc = unsafe {
                    ffi::session::box_iproto_send(session.id as u64, header_start, header_end, body_start, body_end)
                };

                if rc != 0 {
                    unsafe { ffi::tnt::box_txn_rollback(); }
                    let err = unsafe { handle_tarantool_error() };
                    println!("RS | iproto_send() error: {}", err.message);
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

                println!("RS | iproto_send() successful for session_id: {}", session.id);
                let _ = tx.send(Ok(()));
            }),
        });

        tx_push(task);
        tx_flush();

        rx.await.unwrap_or_else(|_| Err(BoxError::new("Failed to receive response from TX thread", "internal error", 500)))
    }

    async fn broadcast(&mut self, key: String, value: String) -> Result<(), BoxError> {
        println!("RS | broadcast() called for key: {}", key);
    
        if key.is_empty() {
            return Err(BoxError::new("Key is empty", "validation error", 1));
        }
        if value.is_empty() {
            return Err(BoxError::new("Value is empty", "validation error", 1));
        }
    
        let key_heap = key.clone();
        let value_heap = value.clone();
        let (tx, rx) = oneshot::channel();
    
        let task = Box::new(TxCtx {
            cb: Box::new(move || {
                println!("RS | Executing broadcast in TX thread...");
    
                unsafe {
                    if let Err(err) = begin_transaction() {
                        let _ = tx.send(Err(err));
                        return;
                    }
                }

                let mut hasher = Sha256::new();
                hasher.update(value_heap.as_bytes());
                let hash = hasher.finalize();
                let hash_str = format!("{:x}", hash);
                let var_name = format!("__wasm_{}", hash_str);
                println!("{}", var_name);
                
                let lua = tarantool::lua_state();
                lua.exec_with(&format!("{} = {};", var_name, value_heap), ()).unwrap();

                let broadcast_result = lua.exec_with(&format!("box.broadcast(..., {})", var_name), key_heap);

                if let Err(err) = broadcast_result {
                    unsafe { ffi::tnt::box_txn_rollback(); }
                    let err_message = format!("Lua error during broadcast: {:?}", err);
                    println!("RS | broadcast() error: {}", err_message);
                    let _ = tx.send(Err(BoxError::new(&err_message, "lua error", 500)));
                    return;
                }
    
                unsafe {
                    if let Err(err) = commit_transaction() {
                        ffi::tnt::box_txn_rollback();
                        let _ = tx.send(Err(err));
                        return;
                    }
                }
    
                println!("RS | broadcast() successful");
                let _ = tx.send(Ok(()));
            }),
        });
    
        tx_push(task);
        tx_flush();
    
        rx.await.unwrap_or_else(|_| Err(BoxError::new("Failed to receive response from TX thread", "internal error", 500)))
    }
}

