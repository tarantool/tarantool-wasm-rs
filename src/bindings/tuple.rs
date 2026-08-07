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

fn tuple_into_ptr(tuple: tarantool::tuple::Tuple) -> u64 {
    let ptr = tuple.as_ptr() as u64;
    std::mem::forget(tuple);
    ptr
}

fn copy_msgpack_value(tuple: u64, field: *const u8) -> Result<Vec<u8>, BoxError> {
    let mut size = 0;
    let data = unsafe { ffi::tuple::box_tuple_data(tuple as *mut _, &mut size) };
    if data.is_null() {
        return Err(unknown_error("tuple data pointer is null"));
    }

    let start = data as usize;
    let end = start
        .checked_add(size)
        .ok_or_else(|| unknown_error("tuple data range overflow"))?;
    let field_start = field as usize;
    if field_start < start || field_start >= end {
        return Err(unknown_error("tuple field points outside tuple data"));
    }

    let remaining = unsafe { std::slice::from_raw_parts(field, end - field_start) };
    let mut cursor = std::io::Cursor::new(remaining);
    rmpv::decode::read_value(&mut cursor)
        .map_err(|err| unknown_error(format!("invalid tuple field MessagePack: {err}")))?;
    Ok(remaining[..cursor.position() as usize].to_vec())
}

impl box_tuple::Host for SyncWasmStates {
    async fn new(&mut self, data: Vec<u8>) -> Result<BoxTuple, BoxError> {
        validate_array(&data)?;
        let ptr = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                Ok(tuple_into_ptr(unsafe {
                    tarantool::tuple::Tuple::from_slice(&data)
                }))
            })
            .await?;
        self.register_tuple(ptr)
    }

    async fn retain(&mut self, t: BoxTuple) -> wasmtime::Result<()> {
        let ptr = self
            .retain_tuple_handle(t)
            .map_err(|err| wasmtime::Error::msg(err.message))?;
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                unsafe { ffi::tuple::box_tuple_ref(ptr as *mut _) };
            })
            .await;
        Ok(())
    }

    async fn release(&mut self, t: BoxTuple) -> wasmtime::Result<()> {
        let ptr = self
            .release_tuple_handle(t)
            .map_err(|err| wasmtime::Error::msg(err.message))?;
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                unsafe { ffi::tuple::box_tuple_unref(ptr as *mut _) };
            })
            .await;
        Ok(())
    }

    async fn field_count(&mut self, t: BoxTuple) -> wasmtime::Result<u32> {
        let ptr = self
            .tuple_ptr(t)
            .map_err(|err| wasmtime::Error::msg(err.message))?;
        Ok(self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                unsafe { ffi::tuple::box_tuple_field_count(ptr as *mut _) }
            })
            .await)
    }

    async fn byte_size(&mut self, t: BoxTuple) -> wasmtime::Result<u64> {
        let ptr = self
            .tuple_ptr(t)
            .map_err(|err| wasmtime::Error::msg(err.message))?;
        Ok(self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                unsafe { ffi::tuple::box_tuple_bsize(ptr as *mut _) as u64 }
            })
            .await)
    }

    async fn to_bytes(&mut self, t: BoxTuple) -> Result<Vec<u8>, BoxError> {
        let ptr = self.tuple_ptr(t)?;
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let size = unsafe { ffi::tuple::box_tuple_bsize(ptr as *mut _) };
                let mut buf = Vec::with_capacity(size);
                let actual_size = unsafe {
                    ffi::tuple::box_tuple_to_buf(ptr as *mut _, buf.as_mut_ptr() as _, size)
                };
                if actual_size < 0 {
                    return Err(handle_tarantool_error());
                }
                unsafe { buf.set_len(actual_size as usize) };
                Ok(buf)
            })
            .await
    }

    async fn format(&mut self, t: BoxTuple) -> wasmtime::Result<TupleFormat> {
        let tuple_ptr = self
            .tuple_ptr(t)
            .map_err(|err| wasmtime::Error::msg(err.message))?;
        let format_ptr = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                let ptr = unsafe { ffi::tuple::box_tuple_format(tuple_ptr as *mut _) };
                unsafe { ffi::tuple::box_tuple_format_ref(ptr) };
                ptr as u64
            })
            .await;
        self.register_tuple_format(format_ptr, true)
            .map_err(|err| wasmtime::Error::msg(err.message))
    }

    async fn field(&mut self, t: BoxTuple, idx: u32) -> Result<Option<TupleField>, BoxError> {
        let ptr = self.tuple_ptr(t)?;
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let field = unsafe { ffi::tuple::box_tuple_field(ptr as *mut _, idx) };
                if field.is_null() {
                    Ok(None)
                } else {
                    copy_msgpack_value(ptr, field).map(Some)
                }
            })
            .await
    }

    async fn field_by_path(
        &mut self,
        t: BoxTuple,
        path: String,
    ) -> Result<Option<TupleField>, BoxError> {
        let ptr = self.tuple_ptr(t)?;
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let c_path = std::ffi::CString::new(path.as_str())
                    .map_err(|_| illegal_params("invalid tuple field path"))?;
                let field = unsafe {
                    ffi::tuple::box_tuple_field_by_path(
                        ptr as *mut _,
                        c_path.as_ptr(),
                        path.len() as u32,
                        0,
                    )
                };
                if field.is_null() {
                    Ok(None)
                } else {
                    copy_msgpack_value(ptr, field).map(Some)
                }
            })
            .await
    }

    async fn update(&mut self, t: BoxTuple, expr: Vec<u8>) -> Result<BoxTuple, BoxError> {
        validate_array(&expr)?;
        let tuple = self.tuple_ptr(t)?;
        let ptr = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                let ptr = unsafe {
                    ffi::tuple::box_tuple_update(
                        tuple as *mut _,
                        expr.as_ptr(),
                        expr.as_ptr().add(expr.len()),
                    )
                };
                if ptr.is_null() {
                    return Err(handle_tarantool_error());
                }
                let rc = unsafe { ffi::tuple::box_tuple_ref(ptr) };
                if rc != 0 {
                    return Err(handle_tarantool_error());
                }
                Ok(ptr as u64)
            })
            .await?;
        self.register_tuple(ptr)
    }

    async fn upsert(&mut self, t: BoxTuple, expr: Vec<u8>) -> Result<BoxTuple, BoxError> {
        validate_array(&expr)?;
        let tuple = self.tuple_ptr(t)?;
        let ptr = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                let ptr = unsafe {
                    ffi::tuple::box_tuple_upsert(
                        tuple as *mut _,
                        expr.as_ptr(),
                        expr.as_ptr().add(expr.len()),
                    )
                };
                if ptr.is_null() {
                    return Err(handle_tarantool_error());
                }
                let rc = unsafe { ffi::tuple::box_tuple_ref(ptr) };
                if rc != 0 {
                    return Err(handle_tarantool_error());
                }
                Ok(ptr as u64)
            })
            .await?;
        self.register_tuple(ptr)
    }

    async fn validate(&mut self, t: BoxTuple, format: TupleFormat) -> Result<(), BoxError> {
        let tuple = self.tuple_ptr(t)?;
        let format = self.tuple_format_ptr(format)?;
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let rc =
                    unsafe { ffi::tuple::box_tuple_validate(tuple as *mut _, format as *mut _) };
                if rc == 0 {
                    Ok(())
                } else {
                    Err(handle_tarantool_error())
                }
            })
            .await
    }

    async fn compare(
        &mut self,
        key_def: KeyDef,
        left: BoxTuple,
        right: BoxTuple,
    ) -> wasmtime::Result<i32> {
        let key_def = self
            .key_def_ptr(key_def)
            .map_err(|err| wasmtime::Error::msg(err.message))?;
        let left = self
            .tuple_ptr(left)
            .map_err(|err| wasmtime::Error::msg(err.message))?;
        let right = self
            .tuple_ptr(right)
            .map_err(|err| wasmtime::Error::msg(err.message))?;
        Ok(self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                unsafe {
                    ffi::tuple::box_tuple_compare(
                        left as *mut _,
                        right as *mut _,
                        key_def as *mut _,
                    )
                }
            })
            .await)
    }

    async fn compare_with_key(
        &mut self,
        tuple: BoxTuple,
        key: Vec<u8>,
        key_def: KeyDef,
    ) -> Result<i32, BoxError> {
        validate_array(&key)?;
        let tuple = self.tuple_ptr(tuple)?;
        let key_def = self.key_def_ptr(key_def)?;
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let mut key_size = 0;
                let rc = unsafe {
                    ffi::tuple::box_key_def_validate_key(
                        key_def as *const _,
                        key.as_ptr() as *const _,
                        &mut key_size,
                    )
                };
                if rc != 0 {
                    return Err(handle_tarantool_error());
                }
                Ok(unsafe {
                    ffi::tuple::box_tuple_compare_with_key(
                        tuple as *mut _,
                        key.as_ptr() as *const _,
                        key_def as *mut _,
                    )
                })
            })
            .await
    }
}

// SAFETY: native tuple iterators are only accessed and destroyed on the TX
// thread. The wrapper only transports their addresses between executors.
unsafe impl Send for TupleIteratorImpl {}

impl Drop for TupleIteratorImpl {
    fn drop(&mut self) {
        if self.iter.is_null() {
            return;
        }
        let iter = SendPtr(std::mem::replace(&mut self.iter, std::ptr::null_mut()));
        let tuple = SendPtr(std::mem::replace(&mut self.tuple, std::ptr::null_mut()));
        run_on_tx_detached(move || unsafe {
            debug_assert_tx_thread();
            ffi::tuple::box_tuple_iterator_free(iter.get());
            ffi::tuple::box_tuple_unref(tuple.get());
        });
    }
}

#[derive(Clone, Copy)]
struct SendPtr<T>(*mut T);

impl<T> SendPtr<T> {
    fn get(self) -> *mut T {
        self.0
    }
}

// SAFETY: this wrapper only transports a pointer to the TX thread.
unsafe impl<T> Send for SendPtr<T> {}

impl box_tuple::HostIterator for SyncWasmStates {
    async fn new(&mut self, tuple: BoxTuple) -> Result<Resource<TupleIteratorImpl>, BoxError> {
        let tuple = self.tuple_ptr(tuple)?;
        let (iter, tuple) = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                let iter = unsafe { ffi::tuple::box_tuple_iterator(tuple as *mut _) };
                if iter.is_null() {
                    return Err(handle_tarantool_error());
                }
                let rc = unsafe { ffi::tuple::box_tuple_ref(tuple as *mut _) };
                if rc != 0 {
                    unsafe { ffi::tuple::box_tuple_iterator_free(iter) };
                    return Err(handle_tarantool_error());
                }
                Ok((SendPtr(iter), SendPtr(tuple as *mut _)))
            })
            .await?;

        let iterator = TupleIteratorImpl {
            iter: iter.get(),
            tuple: tuple.get(),
        };
        let mut guard = self.state.lock().map_err(|_| state_lock_poisoned())?;
        guard
            .table
            .push(iterator)
            .map_err(|err| unknown_error(err.to_string()))
    }

    async fn position(&mut self, self_: Resource<box_tuple::Iterator>) -> wasmtime::Result<u32> {
        let ptr = {
            let mut guard = self
                .state
                .lock()
                .map_err(|_| wasmtime::Error::msg("state lock poisoned"))?;
            let iter = guard.table.get_mut(&self_)?;
            SendPtr(iter.iter)
        };
        Ok(self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                unsafe { ffi::tuple::box_tuple_position(ptr.get()) }
            })
            .await)
    }

    async fn rewind(&mut self, self_: Resource<box_tuple::Iterator>) -> wasmtime::Result<()> {
        let ptr = {
            let mut guard = self
                .state
                .lock()
                .map_err(|_| wasmtime::Error::msg("state lock poisoned"))?;
            let iter = guard.table.get_mut(&self_)?;
            SendPtr(iter.iter)
        };
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                unsafe { ffi::tuple::box_tuple_rewind(ptr.get()) };
            })
            .await;
        Ok(())
    }

    async fn seek(
        &mut self,
        self_: Resource<box_tuple::Iterator>,
        pos: u32,
    ) -> Result<Option<TupleField>, BoxError> {
        let (ptr, tuple) = {
            let mut guard = self.state.lock().map_err(|_| state_lock_poisoned())?;
            let iter = guard
                .table
                .get_mut(&self_)
                .map_err(|err| unknown_error(err.to_string()))?;
            (SendPtr(iter.iter), SendPtr(iter.tuple))
        };
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let field = unsafe { ffi::tuple::box_tuple_seek(ptr.get(), pos) };
                if field.is_null() {
                    Ok(None)
                } else {
                    copy_msgpack_value(tuple.get() as u64, field as *const u8).map(Some)
                }
            })
            .await
    }

    async fn next(
        &mut self,
        self_: Resource<box_tuple::Iterator>,
    ) -> Result<Option<TupleField>, BoxError> {
        let (ptr, tuple) = {
            let mut guard = self.state.lock().map_err(|_| state_lock_poisoned())?;
            let iter = guard
                .table
                .get_mut(&self_)
                .map_err(|err| unknown_error(err.to_string()))?;
            (SendPtr(iter.iter), SendPtr(iter.tuple))
        };
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let field = unsafe { ffi::tuple::box_tuple_next(ptr.get()) };
                if field.is_null() {
                    Ok(None)
                } else {
                    copy_msgpack_value(tuple.get() as u64, field as *const u8).map(Some)
                }
            })
            .await
    }

    async fn drop(&mut self, self_: Resource<box_tuple::Iterator>) -> Result<(), wasmtime::Error> {
        let mut iter = {
            let mut guard = self
                .state
                .lock()
                .map_err(|_| wasmtime::Error::msg("state lock poisoned"))?;
            guard.table.delete(self_)?
        };
        let iter_ptr = SendPtr(std::mem::replace(&mut iter.iter, std::ptr::null_mut()));
        let tuple_ptr = SendPtr(std::mem::replace(&mut iter.tuple, std::ptr::null_mut()));
        self.executor()
            .execute(move || unsafe {
                debug_assert_tx_thread();
                ffi::tuple::box_tuple_iterator_free(iter_ptr.get());
                ffi::tuple::box_tuple_unref(tuple_ptr.get());
            })
            .await;
        Ok(())
    }
}
