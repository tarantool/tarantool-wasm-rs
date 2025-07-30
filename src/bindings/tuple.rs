use crate::bindings::error::handle_tarantool_error;
use crate::bindings::tarantool::tarantool::box_tuple;
use crate::bindings::tarantool::tarantool::types::{BoxError, BoxTuple, KeyDef, TupleFormat};
use crate::bindings::types::validate_msgpack_array;
use crate::bindings::TupleIteratorImpl;
use crate::ffi;
use crate::runtime::tarantool_wasm::SyncWasmStates;

use wasmtime::component::Resource;

pub type TupleField = u64;

impl box_tuple::Host for SyncWasmStates {
    async fn new(&mut self, data: Vec<u8>) -> Result<BoxTuple, BoxError> {
        if let Err(e) = validate_msgpack_array(&data) {
            return Err(BoxError::new(&e, "ValidationError", 1));
        }

        Ok(unsafe { tarantool::tuple::Tuple::from_slice(&data).as_ptr() as BoxTuple })
    }

    async fn update(&mut self, t: BoxTuple, expr: Vec<u8>) -> Result<BoxTuple, BoxError> {
        if let Err(e) = validate_msgpack_array(&expr) {
            return Err(BoxError::new(&e, "ValidationError", 1));
        }

        let ptr = unsafe {
            ffi::tuple::box_tuple_update(
                t as *mut _,
                expr.as_ptr() as *const _,
                expr.as_ptr().add(expr.len()) as *const _,
            )
        };

        if ptr.is_null() {
            return Err(handle_tarantool_error());
        }
        Ok(ptr as BoxTuple)
    }

    async fn upsert(&mut self, t: BoxTuple, expr: Vec<u8>) -> Result<BoxTuple, BoxError> {
        if let Err(e) = validate_msgpack_array(&expr) {
            return Err(BoxError::new(&e, "ValidationError", 1));
        }

        let ptr = unsafe {
            ffi::tuple::box_tuple_upsert(
                t as *mut _,
                expr.as_ptr() as *const _,
                expr.as_ptr().add(expr.len()) as *const _,
            )
        };

        if ptr.is_null() {
            return Err(handle_tarantool_error());
        }
        Ok(ptr as BoxTuple)
    }

    async fn to_buf(&mut self, t: BoxTuple) -> Result<Vec<u8>, BoxError> {
        let size: usize = unsafe { ffi::tuple::box_tuple_bsize(t as *mut _) as u64 } as usize;
        let mut buf = Vec::with_capacity(size);

        let actual_size =
            unsafe { ffi::tuple::box_tuple_to_buf(t as *mut _, buf.as_ptr() as _, size) };

        if actual_size < 0 {
            return Err(handle_tarantool_error());
        }
        unsafe { buf.set_len(actual_size as usize) };
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
        unsafe { ffi::tuple::box_tuple_format(t as *mut _) as TupleFormat }
    }

    async fn field(&mut self, t: BoxTuple, idx: u32) -> Result<TupleField, BoxError> {
        let field_ptr = unsafe { ffi::tuple::box_tuple_field(t as *mut _, idx) };

        if field_ptr.is_null() {
            return Err(BoxError::new("Field not found", "TupleError", 1));
        }
        Ok(field_ptr as TupleField)
    }

    async fn field_by_path(
        &mut self,
        t: BoxTuple,
        path: String,
        index_base: i32,
    ) -> Result<TupleField, BoxError> {
        let c_path = std::ffi::CString::new(path.clone())
            .map_err(|_| BoxError::new("Invalid path", "ValidationError", 1))?;

        let ptr = unsafe {
            ffi::tuple::box_tuple_field_by_path(
                t as *mut _,
                c_path.as_ptr(),
                path.len() as u32,
                index_base,
            )
        };

        if ptr.is_null() {
            return Err(BoxError::new(
                format!("Field '{path}' not found"),
                "TupleError",
                1,
            ));
        }
        Ok(ptr as TupleField)
    }

    async fn validate(&mut self, t: BoxTuple, format: TupleFormat) -> bool {
        let rc = unsafe { ffi::tuple::box_tuple_validate(t as *mut _, format as *mut _) };
        rc == 0
    }

    async fn compare(&mut self, key_def: KeyDef, left: BoxTuple, right: BoxTuple) -> i32 {
        unsafe { ffi::tuple::box_tuple_compare(left as *mut _, right as *mut _, key_def as *mut _) }
    }

    async fn compare_with_key(&mut self, tuple: BoxTuple, key: Vec<u8>, key_def: KeyDef) -> i32 {
        unsafe {
            ffi::tuple::box_tuple_compare_with_key(
                tuple as *mut _,
                key.as_ptr() as *const _,
                key_def as *mut _,
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

struct SendPtr<T>(*mut T);
unsafe impl<T> Send for SendPtr<T> {}
unsafe impl<T> Sync for SendPtr<T> {}

impl box_tuple::HostTupleIterator for SyncWasmStates {
    async fn new_tuple_iterator(
        &mut self,
        tup: BoxTuple,
    ) -> Result<Resource<TupleIteratorImpl>, BoxError> {
        let send_ptr: SendPtr<ffi::types::BoxTupleIteratorT> = self
            .executor()
            .execute(move || {
                let ptr = unsafe { ffi::tuple::box_tuple_iterator(tup as *mut _) };
                if ptr.is_null() {
                    Err(handle_tarantool_error())
                } else {
                    Ok(SendPtr(ptr))
                }
            })
            .await?;

        let iter_ptr = send_ptr.0;
        let resource = self
            .0
            .lock()
            .unwrap()
            .table
            .push(TupleIteratorImpl { iter: iter_ptr })
            .map_err(|e| BoxError::new(e.to_string(), "ResourceTableError", 1))?;

        Ok(resource)
    }

    async fn position(&mut self, self_: Resource<box_tuple::TupleIterator>) -> u32 {
        let mut guard = self.0.lock().unwrap();
        let iter = guard
            .table
            .get_mut(&self_)
            .map_err(|e| BoxError::new(e.to_string(), "ResourceTableError", 1))
            .unwrap();

        unsafe { ffi::tuple::box_tuple_position(iter.iter) }
    }

    async fn rewind(&mut self, self_: Resource<box_tuple::TupleIterator>) {
        let mut guard = self.0.lock().unwrap();
        let iter = guard
            .table
            .get_mut(&self_)
            .map_err(|e| BoxError::new(e.to_string(), "ResourceTableError", 1))
            .unwrap();

        unsafe { ffi::tuple::box_tuple_rewind(iter.iter) };
    }

    async fn seek(
        &mut self,
        self_: Resource<box_tuple::TupleIterator>,
        pos: u32,
    ) -> Result<TupleField, BoxError> {
        let mut guard = self.0.lock().unwrap();
        let iter = guard
            .table
            .get_mut(&self_)
            .map_err(|e| BoxError::new(e.to_string(), "ResourceTableError", 1))?;

        let res = unsafe { ffi::tuple::box_tuple_seek(iter.iter, pos) };
        Ok(res as TupleField)
    }

    async fn next(
        &mut self,
        self_: Resource<box_tuple::TupleIterator>,
    ) -> Result<TupleField, BoxError> {
        let mut guard = self.0.lock().unwrap();
        let iter = guard
            .table
            .get_mut(&self_)
            .map_err(|e| BoxError::new(e.to_string(), "ResourceTableError", 1))?;

        let res = unsafe { ffi::tuple::box_tuple_next(iter.iter) };
        Ok(res as u64)
    }

    async fn drop(
        &mut self,
        self_: Resource<box_tuple::TupleIterator>,
    ) -> Result<(), wasmtime::Error> {
        self.0.lock().unwrap().table.delete(self_)?;
        Ok(())
    }
}
