use crate::bindings::docs::adder::types::{self, BoxError, BoxTuple, Index, IteratorType};
use crate::tarantool_wasm::SyncWasmStates;
use crate::bindings::error::handle_tarantool_error;
use crate::ffi;
use crate::bindings::IteratorImpl;

use wasmtime::component::Resource;
use wasmtime_wasi::async_trait;
use rmpv::decode::read_value;

pub fn validate_msgpack_array(data: &[u8]) -> Result<(), String> {
    if data.is_empty() {
        return Err("Tuple data is empty".to_string());
    }
    match read_value(&mut &data[..]) {
        Ok(val) if val.is_array() => Ok(()),
        Ok(other) => Err(format!("Expected MsgPack array, got {:?}", other)),
        Err(e) => Err(format!("Invalid MsgPack data: {}", e)),
    }
}

unsafe impl Send for IteratorImpl {}
unsafe impl Sync for IteratorImpl {}

impl Drop for IteratorImpl {
    fn drop(&mut self) {
        unsafe {
            if !self.iter.is_null() {
                ffi::index::box_iterator_free(self.iter);
            }
        }
    }
}

pub fn to_c_iterator(iterator_type: IteratorType) -> i32 {
    match iterator_type {
        IteratorType::IterEq => 0,
        IteratorType::IterReq => 1,
        IteratorType::IterAll => 2,
        IteratorType::IterLt => 3,
        IteratorType::IterLe => 4,
        IteratorType::IterGe => 5,
        IteratorType::IterGt => 6,
        IteratorType::IterBitsAllSet => 7,
        IteratorType::IterBitsAnySet => 8,
        IteratorType::IterBitsAllNotSet => 9,
        IteratorType::IterOverlaps => 10,
        IteratorType::IterNeighbor => 11,
        IteratorType::IterNp => 12,
        IteratorType::IterPp => 13,
    }
}

#[async_trait]
impl types::Host for SyncWasmStates {
}

#[async_trait]
impl types::HostIterator for SyncWasmStates {
    async fn new_iterator(
        &mut self,
        index: Index,
        iterator_type: IteratorType,
        key: Vec<u8>,
    ) -> Result<Resource<IteratorImpl>, BoxError> {
        let start = key.as_ptr();
        let end = unsafe { start.add(key.len()) };

        let iter_ptr: *mut ffi::types::BoxIteratorT = unsafe {
            ffi::index::box_index_iterator(
                index.space_id as u32,
                index.id as u32,
                to_c_iterator(iterator_type),
                start,
                end,
            )
        };

        if iter_ptr.is_null() {
            return Err(unsafe { handle_tarantool_error() });
        }

        Ok(self.0.lock().unwrap().table.push(IteratorImpl { iter: iter_ptr }).unwrap())
    }

    async fn next(
        &mut self,
        self_: Resource<types::Iterator>,
    ) -> Result<BoxTuple, BoxError> {
        let mut guard = self.0.lock().unwrap();
        let iter = guard.table.get_mut(&self_)
            .map_err(|e| BoxError::new(
                format!("Resource error: {}", e),
                "internal".to_string(),
                2
            ))?;
        
        let mut result_ptr: *mut ffi::types::BoxTupleT = std::ptr::null_mut();
        let res = unsafe { ffi::index::box_iterator_next(iter.iter, &mut result_ptr) };

        if res != 0 {
            let err = unsafe { handle_tarantool_error() };
            return Err(err);
        }

        Ok(result_ptr as u64)
    }

    async fn drop(
        &mut self, 
        self_: Resource<types::Iterator>
    ) -> Result<(), wasmtime::Error> {
        self.0.lock().unwrap().table.delete(self_)?;
        Ok(())
    }
}