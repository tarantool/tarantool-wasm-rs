use crate::tarantool_wasm::SyncWasmStates;
use std::thread;
use wasmtime::component::bindgen;
use wasmtime_wasi::async_trait;
use crate::bindings::docs::adder::tupler;
use crate::bindings::docs::adder::ttbox;
use crate::bindings::docs::adder::say;
use crate::bindings::docs::adder::error;
use crate::bindings::docs::adder::types::{Space, BoxError, Index, LogLevel};

use std::ffi::{CStr, CString};
use std::ptr;
use crate::tx::{tx_push, tx_flush, TxCtx};
use tokio::sync::oneshot;
use hex;

pub type BoxTuple = u64;

use tarantool::space::Space as TarantoolSpace;
use tarantool::error::TarantoolError;
use tarantool::tuple::Tuple;
use tarantool::log::{self, SayLevel};

bindgen!({
    path: "adder.wit",
    world: "adder",
    async: true,
    with: {
        "docs:adder/tupler/space": TtSpace,
    }
});

extern "C" {
    fn box_schema_version() -> u32;
    fn box_index_id_by_name(space_id: u32, name: *const u8, len: u32) -> u32;
    fn box_insert(space_id: u32, tuple: *const u8, tuple_end: *const u8, result: *mut *mut Tuple) -> i32;
    fn box_update(space_id: u32, index_id: u32,
        key: *const u8, key_end: *const u8,
        ops: *const u8, ops_end: *const u8, index_base: i32,
        result: *mut *mut Tuple) -> i32;

    fn box_replace(space_id: u32, tuple: *const u8, tuple_end: *const u8, result: *mut *mut Tuple) -> i32;

    fn box_upsert(space_id: u32, index_id: u32,
        tuple: *const u8, tuple_end: *const u8,
        ops: *const u8, ops_end: *const u8,
        index_base: i32,
        result: *mut *mut Tuple) -> i32;

    fn box_delete(
        space_id: u32,
        index_id: u32,
        key: *const u8,
        key_end: *const u8,
        result: *mut *mut Tuple,
    ) -> i32;

    fn box_error_last() -> *const TarantoolError;
    fn box_error_type(error: *const TarantoolError) -> *const i8;
    fn box_error_code(error: *const TarantoolError) -> u32;
    fn box_error_message(error: *const TarantoolError) -> *const i8;
    fn box_error_clear();
    fn box_error_set(file: *const i8, line: u32, code: u32, format: *const i8, ...);
}

pub struct TtSpace {
    space_id: u32
}


extern "C" {
    fn box_txn_begin() -> i32;
    fn box_txn_commit() -> i32;
    fn box_txn_rollback() -> i32;
}

#[async_trait]
impl tupler::Host for SyncWasmStates {
    async fn get_tuple(&mut self, name: String, value: Vec<u8>) -> tupler::TtTuple {
        let current_thread = thread::current();
        println!("Current thread name: {:?}", current_thread.name());
        self.yield_now().await;
        let current_thread = thread::current();
        println!("Current thread name: {:?}", current_thread.name());

        tupler::TtTuple {name, value, status: true}
    }
}

#[async_trait]
impl tupler::HostSpace for SyncWasmStates {
    async fn new(&mut self, name: String) -> wasmtime::component::Resource<TtSpace> {
        let space_id = if name == "test" { 512 }  else  { 0 };
        self.0.lock().unwrap().table.push(TtSpace{space_id}).unwrap()
    }

    async fn insert(&mut self, _self_: wasmtime::component::Resource<TtSpace>, tup: tupler::TtTuple) -> Result<tupler::TtTuple, String> {

        println!("TX");
        let current_thread = thread::current();
        println!("Current thread name: {:?}", current_thread.name());
        self.yield_now().await;
        let current_thread = thread::current();
        println!("Current thread name: {:?}", current_thread.name());
        Ok(tup)
    }

    async fn drop(&mut self, self_: wasmtime::component::Resource<TtSpace>) -> Result<(), wasmtime::Error> {
        self.0.lock().unwrap().table.delete(self_);
        Ok(())
    }
}

fn convert_error<E: std::fmt::Display>(e: E) -> BoxError {
    BoxError {
        message: e.to_string(),
        type_: "tarantool error".to_string(),
        code: 0,
        payload: None,
        file: None,
        line: None,
    }
}

#[async_trait]
impl ttbox::Host for SyncWasmStates {
    async fn schema_version(&mut self) -> i64 {
        println!("RS | Executing adder::box::schema_version");
        unsafe { box_schema_version() as i64 }
    }

    async fn space_by_name(&mut self, name: String) -> Result<Space, BoxError> {
        println!("RS | Calling tarantool::box_::space_by_name with name: {}", name);
        
        match TarantoolSpace::find(&name) {
            Some(space) => {
                let space_id = space.id() as i32;
                println!("RS | Found space: name = '{}', id = {}", name, space_id);
                Ok(Space { id: space_id })
            }
            None => {
                println!("RS |Space '{}' not found", name);
                Err(convert_error(format!("Space '{}' not found", name)))
            }
        }
    }

    async fn index_by_name(&mut self, space: Space, name: String) -> Result<Index, BoxError> {
        println!("RS | index_by_name('{}') called for space_id: {}", name, space.id);

        let c_name = match CString::new(name.clone()) {
            Ok(cstr) => cstr,
            Err(_) => {
                return Err(BoxError {
                    message: "CString conversion failed".to_string(),
                    type_: "internal error".to_string(),
                    code: 1,
                    payload: None,
                    file: None,
                    line: None,
                });
            }
        };

        let index_id = unsafe {
            box_index_id_by_name(space.id as u32, c_name.as_ptr() as *const u8, name.len() as u32)
        };

        if index_id == u32::MAX {
            println!("RS | Index '{}' not found in space_id {}", name, space.id);
            return Err(BoxError {
                message: format!("Index '{}' not found in space_id {}", name, space.id),
                type_: "tarantool error".to_string(),
                code: 0,
                payload: None,
                file: None,
                line: None,
            });
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
            return Err(BoxError {
                message: "Tuple data is empty".to_string(),
                type_: "validation error".to_string(),
                code: 1,
                payload: None,
                file: None,
                line: None,
            });
        }

        let tup_heap = tup.clone();
        let (tx, rx) = oneshot::channel();

        let task = Box::new(TxCtx {
            cb: Box::new(move || {
                println!("RS | Executing insert in TX thread...");

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

                let start = tup_heap.as_ptr();
                let end = unsafe { start.add(tup_heap.len()) };
                let mut result_tuple: *mut Tuple = ptr::null_mut();

                let rc = unsafe { box_insert(space.id as u32, start, end, &mut result_tuple) };

                if rc != 0 {
                    unsafe { box_txn_rollback(); }

                    let err_ptr = unsafe { box_error_last() };
                    let err_message = if !err_ptr.is_null() {
                        unsafe { CStr::from_ptr(box_error_message(err_ptr)).to_string_lossy().into_owned() }
                    } else {
                        "Unknown Tarantool error".to_string()
                    };

                    println!("RS | Insert error: {}", err_message);
                    let _ = tx.send(Err(BoxError {
                        message: err_message,
                        type_: "tarantool error".to_string(),
                        code: 0,
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

                let tuple_id = result_tuple as u64;
                println!("RS | Insert successful, tuple_id: {}", tuple_id);
                let _ = tx.send(Ok(tuple_id));
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

    async fn update(&mut self, index: Index, key: Vec<u8>, ops: Vec<u8>) -> Result<BoxTuple, BoxError> {
        println!(
            "RS | update() called for space_id: {}, index_id: {}, index_base: {}",
            index.space_id, index.id, index.index_base
        );

        if key.is_empty() || ops.is_empty() {
            return Err(BoxError {
                message: "Key or ops data is empty".to_string(),
                type_: "validation error".to_string(),
                code: 1,
                payload: None,
                file: None,
                line: None,
            });
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

                let key_start = key_heap.as_ptr();
                let key_end = unsafe { key_start.add(key_heap.len()) };
                let ops_start = ops_heap.as_ptr();
                let ops_end = unsafe { ops_start.add(ops_heap.len()) };

                let mut result_tuple: *mut Tuple = ptr::null_mut();

                println!("RS | Calling box_update...");
                let rc = unsafe {
                    box_update(
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
                    unsafe { box_txn_rollback(); }

                    let err_ptr = unsafe { box_error_last() };
                    let err_message = if !err_ptr.is_null() {
                        unsafe { CStr::from_ptr(box_error_message(err_ptr)).to_string_lossy().into_owned() }
                    } else {
                        "Unknown Tarantool error".to_string()
                    };

                    println!("RS | Update error: {}", err_message);
                    let _ = tx.send(Err(BoxError {
                        message: err_message,
                        type_: "tarantool error".to_string(),
                        code: 0,
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

                let tuple_id = result_tuple as u64;
                println!("RS | Update successful, tuple_id: {}", tuple_id);
                let _ = tx.send(Ok(tuple_id));
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

    async fn replace(&mut self, space: Space, tup: Vec<u8>) -> Result<BoxTuple, BoxError> {
        println!("RS | replace() called for space_id: {}", space.id);

        if tup.is_empty() {
            return Err(BoxError {
                message: "Tuple data is empty".to_string(),
                type_: "validation error".to_string(),
                code: 1,
                payload: None,
                file: None,
                line: None,
            });
        }

        println!("RS | replace() tuple (MsgPack): {:?}", hex::encode(&tup));

        let tup_heap = tup.clone();
        let (tx, rx) = oneshot::channel();

        let task = Box::new(TxCtx {
            cb: Box::new(move || {
                println!("RS | Executing replace in TX thread...");

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

                let tup_start = tup_heap.as_ptr();
                let tup_end = unsafe { tup_start.add(tup_heap.len()) };
                let mut result_tuple: *mut Tuple = ptr::null_mut();

                println!("RS | Calling box_replace...");
                let rc = unsafe { box_replace(space.id as u32, tup_start, tup_end, &mut result_tuple) };

                if rc != 0 {
                    unsafe { box_txn_rollback(); }

                    let err_ptr = unsafe { box_error_last() };
                    let err_message = if !err_ptr.is_null() {
                        unsafe { CStr::from_ptr(box_error_message(err_ptr)).to_string_lossy().into_owned() }
                    } else {
                        "Unknown Tarantool error".to_string()
                    };

                    println!("RS | Update error: {}", err_message);
                    let _ = tx.send(Err(BoxError {
                        message: err_message,
                        type_: "tarantool error".to_string(),
                        code: 0,
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

                let tuple_id = result_tuple as u64;
                println!("RS | Replace successful, tuple_id: {}", tuple_id);
                let _ = tx.send(Ok(tuple_id));
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

    async fn upsert(&mut self, index: Index, key: Vec<u8>, ops: Vec<u8>) -> Result<BoxTuple, BoxError> {
        println!(
            "RS | upsert() called for space_id: {}, index_id: {}, index_base: {}",
            index.space_id, index.id, index.index_base
        );

        if key.is_empty() || ops.is_empty() {
            return Err(BoxError {
                message: "Key or ops data is empty".to_string(),
                type_: "validation error".to_string(),
                code: 1,
                payload: None,
                file: None,
                line: None,
            });
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

                let key_start = key_heap.as_ptr();
                let key_end = unsafe { key_start.add(key_heap.len()) };
                let ops_start = ops_heap.as_ptr();
                let ops_end = unsafe { ops_start.add(ops_heap.len()) };

                let mut result_tuple: *mut Tuple = ptr::null_mut();

                println!("RS | Calling box_upsert...");
                let rc = unsafe {
                    box_upsert(
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
                    unsafe { box_txn_rollback(); }

                    let err_ptr = unsafe { box_error_last() };
                    let err_message = if !err_ptr.is_null() {
                        unsafe { CStr::from_ptr(box_error_message(err_ptr)).to_string_lossy().into_owned() }
                    } else {
                        "Unknown Tarantool error".to_string()
                    };

                    println!("RS | Update error: {}", err_message);
                    let _ = tx.send(Err(BoxError {
                        message: err_message,
                        type_: "tarantool error".to_string(),
                        code: 0,
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

                let tuple_id = result_tuple as u64;
                println!("RS | Upsert successful, tuple_id: {}", tuple_id);
                let _ = tx.send(Ok(tuple_id));
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

    async fn tt_delete(&mut self, index: Index, key: Vec<u8>) -> Result<BoxTuple, BoxError> {
        println!(
            "RS | delete() called for space_id: {}, index_id: {}",
            index.space_id, index.id
        );
    
        if key.is_empty() {
            return Err(BoxError {
                message: "Key data is empty".to_string(),
                type_: "validation error".to_string(),
                code: 1,
                payload: None,
                file: None,
                line: None,
            });
        }
    
        println!("RS | delete() key (MsgPack): {:?}", hex::encode(&key));
    
        let key_heap = key.clone();
        let (tx, rx) = oneshot::channel();
    
        let task = Box::new(TxCtx {
            cb: Box::new(move || {
                println!("RS | Executing delete in TX thread...");
    
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
    
                let key_start = key_heap.as_ptr();
                let key_end = unsafe { key_start.add(key_heap.len()) };
    
                let mut result_tuple: *mut Tuple = ptr::null_mut();
    
                println!("RS | Calling box_delete...");
                let rc = unsafe {
                    box_delete(
                        index.space_id as u32,
                        index.id as u32,
                        key_start,
                        key_end,
                        &mut result_tuple,
                    )
                };
    
                if rc != 0 {
                    unsafe { box_txn_rollback(); }

                    let err_ptr = unsafe { box_error_last() };
                    let err_message = if !err_ptr.is_null() {
                        unsafe { CStr::from_ptr(box_error_message(err_ptr)).to_string_lossy().into_owned() }
                    } else {
                        "Unknown Tarantool error".to_string()
                    };

                    println!("RS | Update error: {}", err_message);
                    let _ = tx.send(Err(BoxError {
                        message: err_message,
                        type_: "tarantool error".to_string(),
                        code: 0,
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
    
                let tuple_id = result_tuple as u64;
                println!("RS | Delete successful, tuple_id: {}", tuple_id);
                let _ = tx.send(Ok(tuple_id));
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

    async fn truncate(&mut self, space: Space) -> Result<(), BoxError> {
        println!("RS | truncate() called for space_id: {}", space.id);
    
        let (tx, rx) = oneshot::channel();
    
        let task = Box::new(TxCtx {
            cb: Box::new(move || {
                println!("RS | Executing truncate in TX thread...");
    
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
    
                let rc = unsafe { box_truncate(space.id as u32) };
    
                if rc != 0 {
                    unsafe { box_txn_rollback(); }

                    let err_ptr = unsafe { box_error_last() };
                    let err_message = if !err_ptr.is_null() {
                        unsafe { CStr::from_ptr(box_error_message(err_ptr)).to_string_lossy().into_owned() }
                    } else {
                        "Unknown Tarantool error".to_string()
                    };

                    println!("RS | Update error: {}", err_message);
                    let _ = tx.send(Err(BoxError {
                        message: err_message,
                        type_: "tarantool error".to_string(),
                        code: 0,
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
    
                println!("RS | Truncate successful");
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
}


impl From<LogLevel> for SayLevel {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::SFatal => SayLevel::Fatal,
            LogLevel::SSyserror => SayLevel::System,
            LogLevel::SError => SayLevel::Error,
            LogLevel::SCrit => SayLevel::Crit,
            LogLevel::SWarn => SayLevel::Warn,
            LogLevel::SInfo => SayLevel::Info,
            LogLevel::SVerbose => SayLevel::Verbose,
            LogLevel::SDebug => SayLevel::Debug,
        }
    }
}

#[async_trait]
impl say::Host for SyncWasmStates {
    async fn say(&mut self, level: LogLevel, msg: String) {
        let log_level: SayLevel = level.into();
        log::say(log_level, file!(), line!() as i32, None, &msg);
    }

    async fn say_error(&mut self, msg: String) {
        log::say(SayLevel::Error, file!(), line!() as i32, None, &msg);
    }

    async fn say_crit(&mut self, msg: String) {
        log::say(SayLevel::Crit, file!(), line!() as i32, None, &msg);
    }

    async fn say_warn(&mut self, msg: String) {
        log::say(SayLevel::Warn, file!(), line!() as i32, None, &msg);
    }

    async fn say_info(&mut self, msg: String) {
        log::say(SayLevel::Info, file!(), line!() as i32, None, &msg);
    }

    async fn say_verbose(&mut self, msg: String) {
        log::say(SayLevel::Verbose, file!(), line!() as i32, None, &msg);
    }

    async fn say_debug(&mut self, msg: String) {
        log::say(SayLevel::Debug, file!(), line!() as i32, None, &msg);
    }

    async fn say_syserror(&mut self, msg: String) {
        log::say(SayLevel::System, file!(), line!() as i32, None, &msg);
    }
}


#[async_trait]
impl error::Host for SyncWasmStates {
    async fn new(&mut self, message: String, type_: String, code: u32) -> BoxError {
        BoxError {
            message: message,
            type_: type_,
            code: code,
            payload: None,
            file: None,
            line: None,
        }
    }

    async fn set(&mut self, err: BoxError) {
        println!("RS | Setting error:");
        println!("RS | - Type: {}", err.type_);
        println!("RS | - Code: {}", err.code);
        println!("RS | - Message: {}", err.message);

        let file = CString::new("rust").unwrap();
        let format = CString::new(err.message.clone()).unwrap();

        unsafe {
            box_error_set(file.as_ptr(), 0, err.code, format.as_ptr());
        }

        println!("RS | Error set successfully");
    }

    async fn last(&mut self) -> Option<BoxError> {
        unsafe {
            let err_ptr = box_error_last();
            if err_ptr.is_null() {
                return None;
            }

            let message = CStr::from_ptr(box_error_message(err_ptr))
                .to_string_lossy()
                .into_owned();
            let type_ = CStr::from_ptr(box_error_type(err_ptr))
                .to_string_lossy()
                .into_owned();
            let code = box_error_code(err_ptr);

            Some(BoxError {
                    message: message,
                    type_: type_,
                    code: code,
                    payload: None,
                    file: None,
                    line: None,
            })
        }
    }

    async fn clear(&mut self) {
        unsafe {
            box_error_clear();
        }
    }

    async fn to_string(&mut self, err: BoxError) -> String {
        format!("[{}] {}: {}", err.code, err.type_, err.message)
    }
}
