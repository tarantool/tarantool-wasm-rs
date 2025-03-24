use crate::bindings::docs::adder::index;
use crate::bindings::docs::adder::types::{BoxError, BoxTuple, Index, IteratorType};
use crate::tarantool_wasm::SyncWasmStates;
use crate::bindings::error::handle_tarantool_error;
use crate::ffi;

use wasmtime_wasi::async_trait;

#[async_trait]
impl index::Host for SyncWasmStates {
    async fn len(&mut self, index: Index) -> Result<i64, BoxError> {
        let len = unsafe{ ffi::index::box_index_len(index.space_id as u32, index.id as u32) };
        if len < 0 {
            let err: BoxError = unsafe { handle_tarantool_error() };
            println!("RS | Error index::len {}", err.message);
            Err(err)
        } else {
            Ok(len as i64)
        }
    }

    async fn bsize(&mut self, index: Index) -> Result<i64, BoxError> {
        let len = unsafe{ ffi::index::box_index_bsize(index.space_id as u32, index.id as u32) };
        if len < 0 {
            let err: BoxError = unsafe { handle_tarantool_error() };
            println!("RS | Error index::bsize {}", err.message);
            Err(err)
        } else {
            Ok(len as i64)
        }
    }

    async fn random(&mut self, index: Index, rnd: u32) -> Result<BoxTuple, BoxError> {
        let mut result_ptr: *mut ffi::types::BoxTupleT = std::ptr::null_mut();
        let res = unsafe {
            ffi::index::box_index_random(
                index.space_id as u32,
                index.id as u32,
                rnd,
                &mut result_ptr)
        };
        if res != 0 {
            let err: BoxError = unsafe { handle_tarantool_error() };
            println!("RS | Error index::random {}", err.message);
            Err(err)
        } else {
            Ok(result_ptr as BoxTuple)
        }
    }

    async fn get(&mut self, index: Index, key: Vec<u8>) -> Result<BoxTuple, BoxError> {

        if key.is_empty() {
            return Err(BoxError::new("Key is empty", "validation error", 1));
        }
        let key_start = key.as_ptr();
        let key_end = unsafe { key_start.add(key.len()) };
        let mut result_ptr: *mut ffi::types::BoxTupleT = std::ptr::null_mut();

        let res = unsafe {
            ffi::index::box_index_get(
                index.space_id as u32,
                index.id as u32,
                key_start,
                key_end,
                &mut result_ptr
            )
        };

        if res != 0 {
            let err: BoxError = unsafe { handle_tarantool_error() };
            println!("RS | Error index::get {}", err.message);
            Err(err)
        } else {
            Ok(result_ptr as BoxTuple)
        }
    }
    
    async fn min(&mut self, index: Index, key: Vec<u8>) -> Result<BoxTuple, BoxError> {

        if key.is_empty() {
            return Err(BoxError::new("Key is empty", "validation error", 1));
        }
        let key_start = key.as_ptr();
        let key_end = unsafe { key_start.add(key.len()) };
        let mut result_ptr: *mut ffi::types::BoxTupleT = std::ptr::null_mut();

        let res = unsafe {
            ffi::index::box_index_min(
                index.space_id as u32,
                index.id as u32,
                key_start,
                key_end,
                &mut result_ptr
            )
        };

        if res != 0 {
            let err: BoxError = unsafe { handle_tarantool_error() };
            println!("RS | Error index::min {}", err.message);
            Err(err)
        } else {
            Ok(result_ptr as BoxTuple)
        }
    }

    async fn max(&mut self, index: Index, key: Vec<u8>) -> Result<BoxTuple, BoxError> {

        if key.is_empty() {
            return Err(BoxError::new("Key is empty", "validation error", 1));
        }
        let key_start = key.as_ptr();
        let key_end = unsafe { key_start.add(key.len()) };
        let mut result_ptr: *mut ffi::types::BoxTupleT = std::ptr::null_mut();

        let res = unsafe {
            ffi::index::box_index_max(
                index.space_id as u32,
                index.id as u32,
                key_start,
                key_end,
                &mut result_ptr
            )
        };

        if res != 0 {
            let err: BoxError = unsafe { handle_tarantool_error() };
            println!("RS | Error index::max {}", err.message);
            Err(err)
        } else {
            Ok(result_ptr as BoxTuple)
        }
    }

    async fn count(&mut self, index: Index, iter_type: IteratorType, key: Vec<u8>) -> Result<i64, BoxError> {

        if key.is_empty() {
            return Err(BoxError::new("Key is empty", "validation error", 1));
        }
        let key_start = key.as_ptr();
        let key_end = unsafe { key_start.add(key.len()) };

        let size = unsafe {
            ffi::index::box_index_count(
                index.space_id as u32,
                index.id as u32,
                crate::bindings::types::to_c_iterator(iter_type),
                key_start,
                key_end,
            )
        };

        if size < 0 {
            let err: BoxError = unsafe { handle_tarantool_error() };
            println!("RS | Error index::max {}", err.message);
            Err(err)
        } else {
            Ok(size as i64)
        }
    }


}