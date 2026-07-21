use crate::bindings::error::handle_tarantool_error;
use crate::bindings::helpers::{
    debug_assert_tx_thread, illegal_params, state_lock_poisoned, unknown_error, validate_array,
};
use crate::bindings::tarantool::tarantool::box_tuple;
use crate::bindings::tarantool::tarantool::types::{
    BoxError, BoxTuple, KeyDef, TupleField, TupleFormat,
};
use crate::bindings::TupleIteratorImpl;
use crate::ffi;
use crate::runtime::tarantool_wasm::SyncWasmStates;
use crate::runtime::tx::run_on_tx_detached;

use wasmtime::component::Resource;

use tarantool::error::TarantoolErrorCode;

impl box_tuple::Host for SyncWasmStates {
    async fn new(&mut self, data: Vec<u8>) -> Result<BoxTuple, BoxError> {
        validate_array(&data)?;
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                Ok(unsafe {
                    BoxTuple {
                        ptr: tarantool::tuple::Tuple::from_slice(&data).as_ptr() as u64,
                    }
                })
            })
            .await
    }

    async fn update(&mut self, t: BoxTuple, expr: Vec<u8>) -> Result<BoxTuple, BoxError> {
        validate_array(&expr)?;

        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let ptr = unsafe {
                    ffi::tuple::box_tuple_update(
                        t.ptr as *mut _,
                        expr.as_ptr() as *const _,
                        expr.as_ptr().add(expr.len()) as *const _,
                    )
                };
                if ptr.is_null() {
                    return Err(handle_tarantool_error());
                }
                Ok(BoxTuple { ptr: ptr as u64 })
            })
            .await
    }

    async fn upsert(&mut self, t: BoxTuple, expr: Vec<u8>) -> Result<BoxTuple, BoxError> {
        validate_array(&expr)?;

        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let ptr = unsafe {
                    ffi::tuple::box_tuple_upsert(
                        t.ptr as *mut _,
                        expr.as_ptr() as *const _,
                        expr.as_ptr().add(expr.len()) as *const _,
                    )
                };
                if ptr.is_null() {
                    return Err(handle_tarantool_error());
                }
                Ok(BoxTuple { ptr: ptr as u64 })
            })
            .await
    }

    async fn to_buf(&mut self, t: BoxTuple) -> Result<Vec<u8>, BoxError> {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let size = unsafe { ffi::tuple::box_tuple_bsize(t.ptr as *mut _) };
                let mut buf = Vec::with_capacity(size);
                let actual_size = unsafe {
                    ffi::tuple::box_tuple_to_buf(t.ptr as *mut _, buf.as_mut_ptr() as _, size)
                };
                if actual_size < 0 {
                    return Err(handle_tarantool_error());
                }
                unsafe { buf.set_len(actual_size as usize) };
                Ok(buf)
            })
            .await
    }

    async fn ref_(&mut self, t: BoxTuple) -> i32 {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                unsafe { ffi::tuple::box_tuple_ref(t.ptr as *mut _) }
            })
            .await
    }

    async fn unref(&mut self, t: BoxTuple) {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                unsafe { ffi::tuple::box_tuple_unref(t.ptr as *mut _) };
            })
            .await;
    }

    async fn field_count(&mut self, t: BoxTuple) -> u32 {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                unsafe { ffi::tuple::box_tuple_field_count(t.ptr as *mut _) }
            })
            .await
    }

    async fn bsize(&mut self, t: BoxTuple) -> u64 {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                unsafe { ffi::tuple::box_tuple_bsize(t.ptr as *mut _) as u64 }
            })
            .await
    }

    async fn format(&mut self, t: BoxTuple) -> TupleFormat {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                unsafe {
                    TupleFormat {
                        ptr: ffi::tuple::box_tuple_format(t.ptr as *mut _) as u64,
                    }
                }
            })
            .await
    }

    async fn field(&mut self, t: BoxTuple, idx: u32) -> Result<TupleField, BoxError> {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let field_ptr = unsafe { ffi::tuple::box_tuple_field(t.ptr as *mut _, idx) };
                if field_ptr.is_null() {
                    return Err(BoxError::new(
                        TarantoolErrorCode::FieldMissing as u32,
                        "Field not found",
                    ));
                }
                Ok(TupleField {
                    ptr: field_ptr as u64,
                })
            })
            .await
    }

    async fn field_by_path(
        &mut self,
        t: BoxTuple,
        path: String,
        index_base: i32,
    ) -> Result<TupleField, BoxError> {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let c_path = std::ffi::CString::new(path.as_str())
                    .map_err(|_| illegal_params("Invalid path"))?;
                let ptr = unsafe {
                    ffi::tuple::box_tuple_field_by_path(
                        t.ptr as *mut _,
                        c_path.as_ptr(),
                        path.len() as u32,
                        index_base,
                    )
                };
                if ptr.is_null() {
                    return Err(BoxError::new(
                        TarantoolErrorCode::FieldMissing as u32,
                        format!("Field '{path}' not found"),
                    ));
                }
                Ok(TupleField { ptr: ptr as u64 })
            })
            .await
    }

    async fn validate(&mut self, t: BoxTuple, format: TupleFormat) -> bool {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                unsafe {
                    ffi::tuple::box_tuple_validate(t.ptr as *mut _, format.ptr as *mut _) == 0
                }
            })
            .await
    }

    async fn compare(&mut self, key_def: KeyDef, left: BoxTuple, right: BoxTuple) -> i32 {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                unsafe {
                    ffi::tuple::box_tuple_compare(
                        left.ptr as *mut _,
                        right.ptr as *mut _,
                        key_def.ptr as *mut _,
                    )
                }
            })
            .await
    }

    async fn compare_with_key(&mut self, tuple: BoxTuple, key: Vec<u8>, key_def: KeyDef) -> i32 {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                unsafe {
                    ffi::tuple::box_tuple_compare_with_key(
                        tuple.ptr as *mut _,
                        key.as_ptr() as *const _,
                        key_def.ptr as *mut _,
                    )
                }
            })
            .await
    }
}

// SAFETY: the iterator is only transported as an opaque value. Every operation,
// including destruction, is dispatched to and asserted to run on the TX thread.
unsafe impl Send for TupleIteratorImpl {}

impl Drop for TupleIteratorImpl {
    fn drop(&mut self) {
        if !self.iter.is_null() {
            let ptr = SendPtr(std::mem::replace(&mut self.iter, std::ptr::null_mut()));
            run_on_tx_detached(move || unsafe {
                debug_assert_tx_thread();
                ffi::tuple::box_tuple_iterator_free(ptr.get());
            });
        }
    }
}

#[derive(Clone, Copy)]
struct SendPtr<T>(*mut T);
impl<T> SendPtr<T> {
    fn get(self) -> *mut T {
        self.0
    }
}

// SAFETY: this wrapper only transports the pointer to the TX thread. Its
// pointee is never accessed on a worker thread.
unsafe impl<T> Send for SendPtr<T> {}

impl box_tuple::HostTupleIterator for SyncWasmStates {
    async fn new_tuple_iterator(
        &mut self,
        tup: BoxTuple,
    ) -> Result<Resource<TupleIteratorImpl>, BoxError> {
        let send_ptr: SendPtr<ffi::types::BoxTupleIteratorT> = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                let ptr = unsafe { ffi::tuple::box_tuple_iterator(tup.ptr as *mut _) };
                if ptr.is_null() {
                    Err(handle_tarantool_error())
                } else {
                    Ok(SendPtr(ptr))
                }
            })
            .await?;

        let iter_ptr = send_ptr.0;
        let mut guard = self.state.lock().map_err(|_| state_lock_poisoned())?;
        let resource = guard
            .table
            .push(TupleIteratorImpl { iter: iter_ptr })
            .map_err(|e| unknown_error(e.to_string()))?;

        Ok(resource)
    }

    async fn position(&mut self, self_: Resource<box_tuple::TupleIterator>) -> u32 {
        let ptr = {
            let mut guard = match self.state.lock() {
                Ok(guard) => guard,
                Err(_) => return 0,
            };
            let iter = match guard.table.get_mut(&self_) {
                Ok(iter) => iter,
                Err(_) => return 0,
            };
            SendPtr(iter.iter)
        };
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                unsafe { ffi::tuple::box_tuple_position(ptr.get()) }
            })
            .await
    }

    async fn rewind(&mut self, self_: Resource<box_tuple::TupleIterator>) {
        let ptr = {
            let mut guard = match self.state.lock() {
                Ok(guard) => guard,
                Err(_) => return,
            };
            let iter = match guard.table.get_mut(&self_) {
                Ok(iter) => iter,
                Err(_) => return,
            };
            SendPtr(iter.iter)
        };
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                unsafe { ffi::tuple::box_tuple_rewind(ptr.get()) };
            })
            .await;
    }

    async fn seek(
        &mut self,
        self_: Resource<box_tuple::TupleIterator>,
        pos: u32,
    ) -> Result<TupleField, BoxError> {
        let ptr = {
            let mut guard = self.state.lock().map_err(|_| state_lock_poisoned())?;
            let iter = guard
                .table
                .get_mut(&self_)
                .map_err(|e| unknown_error(e.to_string()))?;
            SendPtr(iter.iter)
        };
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let res = unsafe { ffi::tuple::box_tuple_seek(ptr.get(), pos) };
                Ok(TupleField { ptr: res as u64 })
            })
            .await
    }

    async fn next(
        &mut self,
        self_: Resource<box_tuple::TupleIterator>,
    ) -> Result<TupleField, BoxError> {
        let ptr = {
            let mut guard = self.state.lock().map_err(|_| state_lock_poisoned())?;
            let iter = guard
                .table
                .get_mut(&self_)
                .map_err(|e| unknown_error(e.to_string()))?;
            SendPtr(iter.iter)
        };
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let res = unsafe { ffi::tuple::box_tuple_next(ptr.get()) };
                Ok(TupleField { ptr: res as u64 })
            })
            .await
    }

    async fn drop(
        &mut self,
        self_: Resource<box_tuple::TupleIterator>,
    ) -> Result<(), wasmtime::Error> {
        let mut iter = {
            let mut guard = self
                .state
                .lock()
                .map_err(|_| wasmtime::Error::msg("state lock poisoned"))?;
            guard.table.delete(self_)?
        };
        let ptr = SendPtr(std::mem::replace(&mut iter.iter, std::ptr::null_mut()));
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                unsafe { ffi::tuple::box_tuple_iterator_free(ptr.get()) };
            })
            .await;
        Ok(())
    }
}
