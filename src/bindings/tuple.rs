use crate::bindings::docs::adder::tuple;
use crate::bindings::docs::adder::types::{BoxError, BoxTuple, TupleFormat, KeyDef};
use crate::tarantool_wasm::SyncWasmStates;
use crate::ffi as ffi;
use crate::bindings::error::handle_tarantool_error;
use crate::bindings::TupleIteratorImpl;

use wasmtime_wasi::async_trait;
use wasmtime::component::Resource;

pub type TupleField = u64;

#[async_trait]
impl tuple::Host for SyncWasmStates {
    async fn new(&mut self, format: TupleFormat, data: Vec<u8>) -> Result<BoxTuple, BoxError> {
        let data_ptr = data.as_ptr();
        let data_end = unsafe { data_ptr.add(data.len()) };

        let tuple_ptr = unsafe {
            ffi::tuple::box_tuple_new(format as *mut ffi::types::BoxTupleFormatT, data_ptr as *const _, data_end as *const _)
        };

        if tuple_ptr.is_null() {
            Err(unsafe { handle_tarantool_error() })
        } else {
            Ok(tuple_ptr as BoxTuple)
        }
    }

    async fn update(&mut self, t: BoxTuple, expr: Vec<u8>) -> Result<BoxTuple, BoxError> {
        let ptr = unsafe {
            ffi::tuple::box_tuple_update(
                t as *mut _,
                expr.as_ptr() as *const _,
                expr.as_ptr().add(expr.len()) as *const _
            )
        };

        if ptr.is_null() {
            Err(unsafe { handle_tarantool_error() })
        } else {
            Ok(ptr as u64)
        }
    }

    async fn upsert(&mut self, t: BoxTuple, expr: Vec<u8>) -> Result<BoxTuple, BoxError> {
        let ptr = unsafe {
            ffi::tuple::box_tuple_upsert(
                t as *mut _,
                expr.as_ptr() as *const _,
                expr.as_ptr().add(expr.len()) as *const _
            )
        };

        if ptr.is_null() {
            Err(unsafe { handle_tarantool_error() })
        } else {
            Ok(ptr as u64)
        }
    }

    async fn to_buf(&mut self, t: BoxTuple) -> Result<Vec<u8>, BoxError> {
        let mut size: usize = 128;
        let mut buf = vec![0u8; size];

        let mut written = unsafe {
            ffi::tuple::box_tuple_to_buf(
                t as *mut _,
                buf.as_mut_ptr() as *mut _,
                buf.len()
            )
        };

        if written < 0 {
            return Err(unsafe { handle_tarantool_error() });
        }

        if written as usize > buf.len() {
            // Resize and retry
            size = written as usize;
            buf = vec![0u8; size];
            written = unsafe {
                ffi::tuple::box_tuple_to_buf(
                    t as *mut _,
                    buf.as_mut_ptr() as *mut _,
                    buf.len()
                )
            };

            if written < 0 {
                return Err(unsafe { handle_tarantool_error() });
            }
        }

        buf.truncate(written as usize);
        Ok(buf)
    }

    async fn ref_(&mut self, t: BoxTuple) -> i32 {
        unsafe { ffi::tuple::box_tuple_ref(t as *mut _) }
    }

    async fn unref(&mut self, t: BoxTuple) {
        unsafe { ffi::tuple::box_tuple_unref(t as *mut _) };
    }

    async fn field_count(&mut self, t: BoxTuple) -> u32 {
        unsafe { ffi::tuple::box_tuple_field_count(t as *mut _) }
    }

    async fn bsize(&mut self, t: BoxTuple) -> u64 {
        unsafe { ffi::tuple::box_tuple_bsize(t as *mut _) as u64 }
    }

    async fn format(&mut self, t: BoxTuple) -> TupleFormat {
        unsafe { ffi::tuple::box_tuple_format(t as *mut _) as u64 }
    }

    async fn field(&mut self, t: BoxTuple, idx: u32) -> Result<TupleField, BoxError> {
        let field_ptr = unsafe { ffi::tuple::box_tuple_field(t as *mut _, idx) };
    
        if field_ptr.is_null() {
            Err(BoxError::new("Field not found", "tuple error", 1))
        } else {
            Ok(field_ptr as TupleField)
        }
    }

    async fn field_by_path(&mut self, t: BoxTuple, path: String, index_base: i32) -> Result<TupleField, BoxError> {
        let c_path = std::ffi::CString::new(path.clone())
            .map_err(|_| BoxError::new("Invalid path", "validation error", 2))?;
    
        let ptr = unsafe {
            ffi::tuple::box_tuple_field_by_path(
                t as *mut _,
                c_path.as_ptr(),
                path.len() as u32,
                index_base,
            )
        };
    
        if ptr.is_null() {
            Err(BoxError::new("Field not found", "tuple error", 1))
        } else {
            Ok(ptr as TupleField)
        }
    }
    
    async fn validate(&mut self, t: BoxTuple, format: TupleFormat) -> bool {
        let rc = unsafe {
            ffi::tuple::box_tuple_validate(t as *mut _, format as *mut _)
        };
        rc == 0
    }

    async fn compare(&mut self, key_def: KeyDef, left: BoxTuple, right: BoxTuple) -> i32 {
        unsafe {
            ffi::tuple::box_tuple_compare(
                left as *mut _,
                right as *mut _,
                key_def as *mut _
            )
        }
    }

    async fn compare_with_key(&mut self, tuple: BoxTuple, key: Vec<u8>, key_def: KeyDef) -> i32 {
        unsafe {
            ffi::tuple::box_tuple_compare_with_key(
                tuple as *mut _,
                key.as_ptr() as *const _,
                key_def as *mut _
            )
        }
    }
}

unsafe impl Send for TupleIteratorImpl {}
unsafe impl Sync for TupleIteratorImpl {}

impl Drop for TupleIteratorImpl {
    fn drop(&mut self) {
        unsafe {
            if !self.iter.is_null() {
                ffi::tuple::box_tuple_iterator_free(self.iter);
            }
        }
    }
}

#[async_trait]
impl tuple::HostTupleIterator for SyncWasmStates {
    async fn new_tuple_iterator(&mut self, tup: BoxTuple) -> Result<Resource<TupleIteratorImpl>, BoxError> {
        let iter_ptr: *mut ffi::types::BoxTupleIteratorT = unsafe { ffi::tuple::box_tuple_iterator(tup as *mut _) };
        if iter_ptr.is_null() {
            return Err(unsafe { handle_tarantool_error() });
        }

        Ok(self.0.lock().unwrap().table.push(TupleIteratorImpl { iter: iter_ptr }).unwrap())
    }

    async fn position(&mut self, self_: Resource<tuple::TupleIterator>,) -> u32 {
        let mut guard = self.0.lock().unwrap();
        let iter = guard.table.get_mut(&self_)
            .map_err(|e| BoxError::new(
                format!("Resource error: {}", e),
                "internal".to_string(),
                2
            )).unwrap();

        let res = unsafe { ffi::tuple::box_tuple_position(iter.iter) };
        res
    }

    async fn rewind(&mut self, self_: Resource<tuple::TupleIterator>,) {
        let mut guard = self.0.lock().unwrap();
        let iter = guard.table.get_mut(&self_)
            .map_err(|e| BoxError::new(
                format!("Resource error: {}", e),
                "internal".to_string(),
                2
            )).unwrap();

        unsafe { ffi::tuple::box_tuple_rewind(iter.iter) };
    }

    async fn seek(
        &mut self,
        self_: Resource<tuple::TupleIterator>,
        pos: u32,
    ) -> Result<TupleField, BoxError> {
        let mut guard = self.0.lock().unwrap();
        let iter = guard.table.get_mut(&self_)
            .map_err(|e| BoxError::new(
                format!("Resource error: {}", e),
                "internal".to_string(),
                2
            ))?;

        let res = unsafe { ffi::tuple::box_tuple_seek(iter.iter, pos) };
        if res.is_null() {
            let err = BoxError::new("Iterator end", "iterator error", 3);
            return Err(err);
        }
        Ok(res as TupleField)
    }

    async fn next(
        &mut self,
        self_: Resource<tuple::TupleIterator>,
    ) -> Result<TupleField, BoxError> {
        let mut guard = self.0.lock().unwrap();
        let iter = guard.table.get_mut(&self_)
            .map_err(|e| BoxError::new(
                format!("Resource error: {}", e),
                "internal".to_string(),
                2
            ))?;

        let res = unsafe { ffi::tuple::box_tuple_next(iter.iter) };

        if res.is_null() {
            let err = BoxError::new("Iterator end", "iterator error", 3);
            return Err(err);
        }

        Ok(res as u64)
    }

    async fn drop(
        &mut self,
        self_: Resource<tuple::TupleIterator>
    ) -> Result<(), wasmtime::Error> {
        self.0.lock().unwrap().table.delete(self_)?;
        Ok(())
    }
}
