use crate::bindings::docs::adder::ttbox;
use crate::tarantool_wasm::SyncWasmStates;
use crate::bindings::docs::adder::types::{BoxError, Index, Space};
use crate::tx::{tx_push, tx_flush, TxCtx};
use crate::ffi as ffi;
use crate::bindings::error::handle_tarantool_error;

use wasmtime_wasi::async_trait;
use tokio::sync::oneshot;
use std::ffi::CString;

pub type BoxTuple = u64;

pub unsafe fn begin_transaction() -> Result<(), BoxError> {
    if ffi::tnt::box_txn_begin() != 0 {
        Err(handle_tarantool_error())
    } else {
        Ok(())
    }
}

pub unsafe fn commit_transaction() -> Result<(), BoxError> {
    if ffi::tnt::box_txn_commit() != 0 {
        Err(handle_tarantool_error())
    } else {
        Ok(())
    }
}

#[async_trait]
impl ttbox::Host for SyncWasmStates {
    async fn schema_version(&mut self) -> i64 {
        println!("RS | Executing adder::box::schema_version");
        unsafe { ffi::r#box::box_schema_version() as i64 }
    }

    async fn space_by_name(&mut self, name: String) -> Result<Space, BoxError> {
        println!("RS | Calling box_space_id_by_name with name: {}", name);

        let c_name = match CString::new(name.clone()) {
            Ok(cstr) => cstr,
            Err(_) => {
                return Err(BoxError::new("CString conversion failed", "internal error", 1));
            }
        };

        let space_id = unsafe { ffi::r#box::box_space_id_by_name(c_name.as_ptr(), name.len() as u32) };

        if space_id == ffi::types::BOX_ID_NIL {
            println!("RS | Space '{}' not found", name);
            unsafe { return Err(handle_tarantool_error()); }
        }

        println!("RS | Found space: name = '{}', id = {}", name, space_id);

        Ok(Space { id: space_id as i32 })
    }

    async fn index_by_name(&mut self, space: Space, name: String) -> Result<Index, BoxError> {
        println!("RS | index_by_name('{}') called for space_id: {}", name, space.id);

        let c_name = match CString::new(name.clone()) {
            Ok(cstr) => cstr,
            Err(_) => {
                return Err(BoxError::new("CString conversion failed", "internal error", 1));
            }
        };

        let index_id = unsafe {
            ffi::r#box::box_index_id_by_name(space.id as u32, c_name.as_ptr() as *const u8, name.len() as u32)
        };

        if index_id == ffi::types::BOX_ID_NIL {
            println!("RS | Error retrieving index '{}' for space_id {}", name, space.id);
            unsafe { return Err(handle_tarantool_error()); }
        }

        println!("RS | Found index '{}' with id {} in space_id {}", name, index_id, space.id);

        Ok(Index {
            id: index_id as i32,
            space_id: space.id,
            index_base: 0,
        })
    }

    async fn insert(&mut self, space: Space, tup: Vec<u8>) -> Result<BoxTuple, BoxError> {
        println!("RS | insert() called for space_id: {}", space.id);

        if tup.is_empty() {
            return Err(BoxError::new("Tuple data is empty", "validation error", 1));
        }

        let tup_heap = tup.clone();
        let (tx, rx) = oneshot::channel();

        let task = Box::new(TxCtx {
            cb: Box::new(move || {
                println!("RS | Executing insert in TX thread...");

                unsafe {
                    if let Err(err) = begin_transaction() {
                        let _ = tx.send(Err(err));
                        return;
                    }
                }

                let start = tup_heap.as_ptr();
                let end = unsafe { start.add(tup_heap.len()) };
                let mut result_tuple: *mut ffi::types::BoxTupleT = std::ptr::null_mut();

                let rc = unsafe { ffi::r#box::box_insert(space.id as u32, start, end, &mut result_tuple) };

                if rc != 0 {
                    unsafe { ffi::tnt::box_txn_rollback(); }
                    let err = unsafe { handle_tarantool_error() };
                    println!("RS | Insert error: {}", err.message);
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

                let tuple_id = result_tuple as u64;
                println!("RS | Insert successful, tuple_id: {}", tuple_id);
                let _ = tx.send(Ok(tuple_id));
            }),
        });

        tx_push(task);
        tx_flush();

        rx.await.unwrap_or_else(|_| Err(BoxError::new("Failed to receive response from TX thread", "internal error", 500)))
    }

    async fn update(&mut self, index: Index, key: Vec<u8>, ops: Vec<u8>) -> Result<BoxTuple, BoxError> {
        println!(
            "RS | update() called for space_id: {}, index_id: {}, index_base: {}",
            index.space_id, index.id, index.index_base
        );

        if key.is_empty() || ops.is_empty() {
            return Err(BoxError::new("Key or ops data is empty", "validation error", 1));
        }

        println!("RS | update() key (MsgPack): {:?}", hex::encode(&key));
        println!("RS | update() ops (MsgPack): {:?}", hex::encode(&ops));

        let key_heap = key.clone();
        let ops_heap = ops.clone();
        let (tx, rx) = oneshot::channel();

        let task = Box::new(TxCtx {
            cb: Box::new(move || {
                println!("RS | Executing update in TX thread...");

                unsafe {
                    if let Err(err) = begin_transaction() {
                        let _ = tx.send(Err(err));
                        return;
                    }
                }

                let key_start = key_heap.as_ptr();
                let key_end = unsafe { key_start.add(key_heap.len()) };
                let ops_start = ops_heap.as_ptr();
                let ops_end = unsafe { ops_start.add(ops_heap.len()) };

                let mut result_tuple: *mut ffi::types::BoxTupleT = std::ptr::null_mut();

                println!("RS | Calling box_update...");
                let rc = unsafe {
                    ffi::r#box::box_update(
                        index.space_id as u32,
                        index.id as u32,
                        key_start,
                        key_end,
                        ops_start,
                        ops_end,
                        index.index_base,
                        &mut result_tuple,
                    )
                };

                if rc != 0 {
                    unsafe { ffi::tnt::box_txn_rollback(); }
                    let err = unsafe { handle_tarantool_error() };
                    println!("RS | Update error: {}", err.message);
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

                let tuple_id = result_tuple as u64;
                println!("RS | Update successful, tuple_id: {}", tuple_id);
                let _ = tx.send(Ok(tuple_id));
            }),
        });

        tx_push(task);
        tx_flush();

        rx.await.unwrap_or_else(|_| Err(BoxError::new("Failed to receive response from TX thread", "internal error", 500)))
    }

    async fn replace(&mut self, space: Space, tup: Vec<u8>) -> Result<BoxTuple, BoxError> {
        println!("RS | replace() called for space_id: {}", space.id);

        if tup.is_empty() {
            return Err(BoxError::new("Tuple data is empty", "validation error", 1));
        }

        println!("RS | replace() tuple (MsgPack): {:?}", hex::encode(&tup));

        let tup_heap = tup.clone();
        let (tx, rx) = oneshot::channel();

        let task = Box::new(TxCtx {
            cb: Box::new(move || {
                println!("RS | Executing replace in TX thread...");

                unsafe {
                    if let Err(err) = begin_transaction() {
                        let _ = tx.send(Err(err));
                        return;
                    }
                }

                let tup_start = tup_heap.as_ptr();
                let tup_end = unsafe { tup_start.add(tup_heap.len()) };
                let mut result_tuple: *mut ffi::types::BoxTupleT = std::ptr::null_mut();

                println!("RS | Calling box_replace...");
                let rc = unsafe { ffi::r#box::box_replace(space.id as u32, tup_start, tup_end, &mut result_tuple) };

                if rc != 0 {
                    unsafe { ffi::tnt::box_txn_rollback(); }
                    let err = unsafe { handle_tarantool_error() };
                    println!("RS | Replace error: {}", err.message);
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

                let tuple_id = result_tuple as u64;
                println!("RS | Replace successful, tuple_id: {}", tuple_id);
                let _ = tx.send(Ok(tuple_id));
            }),
        });

        tx_push(task);
        tx_flush();

        rx.await.unwrap_or_else(|_| Err(BoxError::new("Failed to receive response from TX thread", "internal error", 500)))
    }

    async fn upsert(&mut self, index: Index, key: Vec<u8>, ops: Vec<u8>) -> Result<BoxTuple, BoxError> {
        println!(
            "RS | upsert() called for space_id: {}, index_id: {}, index_base: {}",
            index.space_id, index.id, index.index_base
        );

        if key.is_empty() || ops.is_empty() {
            return Err(BoxError::new("Key or ops data is empty", "validation error", 1));
        }

        println!("RS | upsert() key (MsgPack): {:?}", hex::encode(&key));
        println!("RS | upsert() ops (MsgPack): {:?}", hex::encode(&ops));

        let key_heap = key.clone();
        let ops_heap = ops.clone();
        let (tx, rx) = oneshot::channel();

        let task = Box::new(TxCtx {
            cb: Box::new(move || {
                println!("RS | Executing upsert in TX thread...");

                unsafe {
                    if let Err(err) = begin_transaction() {
                        let _ = tx.send(Err(err));
                        return;
                    }
                }

                let key_start = key_heap.as_ptr();
                let key_end = unsafe { key_start.add(key_heap.len()) };
                let ops_start = ops_heap.as_ptr();
                let ops_end = unsafe { ops_start.add(ops_heap.len()) };

                let mut result_tuple: *mut ffi::types::BoxTupleT = std::ptr::null_mut();

                println!("RS | Calling box_upsert...");
                let rc = unsafe {
                    ffi::r#box::box_upsert(
                        index.space_id as u32,
                        index.id as u32,
                        key_start,
                        key_end,
                        ops_start,
                        ops_end,
                        index.index_base,
                        &mut result_tuple,
                    )
                };

                if rc != 0 {
                    unsafe { ffi::tnt::box_txn_rollback(); }
                    let err = unsafe { handle_tarantool_error() };
                    println!("RS | Upsert error: {}", err.message);
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

                let tuple_id = result_tuple as u64;
                println!("RS | Upsert successful, tuple_id: {}", tuple_id);
                let _ = tx.send(Ok(tuple_id));
            }),
        });

        tx_push(task);
        tx_flush();

        rx.await.unwrap_or_else(|_| Err(BoxError::new("Failed to receive response from TX thread", "internal error", 500)))
    }

    async fn tt_delete(&mut self, index: Index, key: Vec<u8>) -> Result<BoxTuple, BoxError> {
        println!(
            "RS | delete() called for space_id: {}, index_id: {}",
            index.space_id, index.id
        );

        if key.is_empty() {
            return Err(BoxError::new("Key data is empty", "validation error", 1));
        }

        println!("RS | delete() key (MsgPack): {:?}", hex::encode(&key));

        let key_heap = key.clone();
        let (tx, rx) = oneshot::channel();

        let task = Box::new(TxCtx {
            cb: Box::new(move || {
                println!("RS | Executing delete in TX thread...");

                unsafe {
                    if let Err(err) = begin_transaction() {
                        let _ = tx.send(Err(err));
                        return;
                    }
                }

                let key_start = key_heap.as_ptr();
                let key_end = unsafe { key_start.add(key_heap.len()) };

                let mut result_tuple: *mut ffi::types::BoxTupleT = std::ptr::null_mut();

                println!("RS | Calling box_delete...");
                let rc = unsafe {
                    ffi::r#box::box_delete(
                        index.space_id as u32,
                        index.id as u32,
                        key_start,
                        key_end,
                        &mut result_tuple,
                    )
                };

                if rc != 0 {
                    unsafe { ffi::tnt::box_txn_rollback(); }
                    let err = unsafe { handle_tarantool_error() };
                    println!("RS | Delete error: {}", err.message);
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

                let tuple_id = result_tuple as u64;
                println!("RS | Delete successful, tuple_id: {}", tuple_id);
                let _ = tx.send(Ok(tuple_id));
            }),
        });

        tx_push(task);
        tx_flush();
    
        rx.await.unwrap_or_else(|_| Err(BoxError::new("Failed to receive response from TX thread", "internal error", 500)))
    }
    

    async fn truncate(&mut self, space: Space) -> Result<(), BoxError> {
        println!("RS | truncate() called for space_id: {}", space.id);

        let (tx, rx) = oneshot::channel();

        let task = Box::new(TxCtx {
            cb: Box::new(move || {
                println!("RS | Executing truncate in TX thread...");

                unsafe {
                    if let Err(err) = begin_transaction() {
                        let _ = tx.send(Err(err));
                        return;
                    }
                }

                let rc = unsafe { ffi::r#box::box_truncate(space.id as u32) };

                if rc != 0 {
                    unsafe { ffi::tnt::box_txn_rollback(); }
                    let err = unsafe { handle_tarantool_error() };
                    println!("RS | Truncate error: {}", err.message);
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

                println!("RS | Truncate successful");
                let _ = tx.send(Ok(()));
            }),
        });

        tx_push(task);
        tx_flush();

        rx.await.unwrap_or_else(|_| Err(BoxError::new("Failed to receive response from TX thread", "internal error", 500)))
    } 
}
