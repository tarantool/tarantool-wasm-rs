use crate::bindings::docs::adder::read_view;
use crate::bindings::error::handle_tarantool_error;
use crate::bindings::helpers::{
    debug_assert_tx_thread, illegal_params, unknown_error, validate_array, BoxRegionGuard,
};
use crate::bindings::tarantool::tarantool::types::{BoxError, IteratorType};
use crate::bindings::types::convert_iterator_type;
use crate::bindings::{RawReadViewImpl, RawReadViewIteratorImpl};
use crate::ffi::raw_read_view;
use crate::runtime::tarantool_wasm::SyncWasmStates;
use crate::runtime::tx::run_on_tx_detached;
use std::ffi::CString;
use std::sync::Arc;
use tarantool::error::TarantoolErrorCode;
use wasmtime::component::Resource;

fn api() -> Result<&'static raw_read_view::Api, BoxError> {
    if !raw_read_view::is_available() {
        return Err(BoxError::new(
            TarantoolErrorCode::Unsupported as u32,
            "raw read-view API requires Tarantool Enterprise Edition",
        ));
    }
    raw_read_view::api().map_err(unknown_error)
}

pub(super) struct RawReadViewInner {
    ptr: usize,
    api: &'static raw_read_view::Api,
}

impl Drop for RawReadViewInner {
    fn drop(&mut self) {
        let ptr = self.ptr;
        let api = self.api;
        run_on_tx_detached(move || unsafe {
            debug_assert_tx_thread();
            api.delete(ptr as *mut raw_read_view::ReadView);
        });
    }
}

pub(super) struct RawReadViewIteratorInner {
    iterator: Box<raw_read_view::Iterator>,
    view: Arc<RawReadViewInner>,
}

impl Drop for RawReadViewIteratorImpl {
    fn drop(&mut self) {
        let Some(inner) = self.inner.take() else {
            return;
        };
        let api = inner.view.api;
        run_on_tx_detached(move || {
            debug_assert_tx_thread();
            let mut inner = inner;
            unsafe { api.iterator_destroy(inner.iterator.as_mut()) };
            drop(inner);
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

// SAFETY: pointers wrapped in SendPtr are only dereferenced on the TX thread.
unsafe impl<T> Send for SendPtr<T> {}

fn copy_result(data: *const u8, size: u32) -> Result<Option<Vec<u8>>, BoxError> {
    if data.is_null() {
        if size == 0 {
            return Ok(None);
        }
        return Err(unknown_error("raw read-view returned a null tuple pointer"));
    }
    Ok(Some(unsafe {
        std::slice::from_raw_parts(data, size as usize).to_vec()
    }))
}

fn lookup_index(
    inner: &RawReadViewInner,
    space_id: u32,
    index_id: u32,
) -> Result<*mut raw_read_view::Index, BoxError> {
    debug_assert_tx_thread();
    let index = unsafe {
        inner.api.index_by_id(
            inner.ptr as *const raw_read_view::ReadView,
            space_id,
            index_id,
        )
    };
    if index.is_null() {
        return Err(handle_tarantool_error());
    }
    Ok(index)
}

impl read_view::Host for SyncWasmStates {
    async fn open(&mut self, name: String) -> Result<Resource<RawReadViewImpl>, BoxError> {
        let api = api()?;
        let name = CString::new(name)
            .map_err(|_| illegal_params("read-view name must not contain NUL bytes"))?;
        let ptr = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                let ptr = unsafe { api.open(&name) };
                if ptr.is_null() {
                    Err(handle_tarantool_error())
                } else {
                    Ok(ptr as usize)
                }
            })
            .await?;
        let view = RawReadViewImpl {
            inner: Arc::new(RawReadViewInner { ptr, api }),
        };
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        state
            .table
            .push(view)
            .map_err(|err| unknown_error(format!("resource table error: {err}")))
    }
}

impl read_view::HostView for SyncWasmStates {
    async fn get(
        &mut self,
        self_: Resource<RawReadViewImpl>,
        space_id: u32,
        index_id: u32,
        key: Vec<u8>,
    ) -> Result<Option<Vec<u8>>, BoxError> {
        validate_array(&key)?;
        let view = {
            let state = self
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            state
                .table
                .get(&self_)
                .map_err(|err| unknown_error(format!("resource error: {err}")))?
                .inner
                .clone()
        };
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let _region = BoxRegionGuard::new();
                let index = lookup_index(&view, space_id, index_id)?;
                let (rc, data, size) = unsafe { view.api.get(index, &key) };
                if rc != 0 {
                    return Err(handle_tarantool_error());
                }
                copy_result(data, size)
            })
            .await
    }

    async fn iterator(
        &mut self,
        self_: Resource<RawReadViewImpl>,
        space_id: u32,
        index_id: u32,
        mode: IteratorType,
        key: Vec<u8>,
    ) -> Result<Resource<RawReadViewIteratorImpl>, BoxError> {
        validate_array(&key)?;
        let mode = convert_iterator_type(mode)? as i32;
        let view = {
            let state = self
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            state
                .table
                .get(&self_)
                .map_err(|err| unknown_error(format!("resource error: {err}")))?
                .inner
                .clone()
        };
        let inner = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                let index = lookup_index(&view, space_id, index_id)?;
                let mut iterator = Box::<raw_read_view::Iterator>::default();
                let rc = unsafe {
                    view.api
                        .iterator_create(iterator.as_mut(), index, mode, &key)
                };
                if rc != 0 {
                    return Err(handle_tarantool_error());
                }
                Ok(RawReadViewIteratorInner { iterator, view })
            })
            .await?;
        let iterator = RawReadViewIteratorImpl { inner: Some(inner) };
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        state
            .table
            .push(iterator)
            .map_err(|err| unknown_error(format!("resource table error: {err}")))
    }

    async fn count(
        &mut self,
        self_: Resource<RawReadViewImpl>,
        space_id: u32,
        index_id: u32,
        mode: IteratorType,
        key: Vec<u8>,
    ) -> Result<u64, BoxError> {
        validate_array(&key)?;
        let mode = convert_iterator_type(mode)? as i32;
        let view = {
            let state = self
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            state
                .table
                .get(&self_)
                .map_err(|err| unknown_error(format!("resource error: {err}")))?
                .inner
                .clone()
        };
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let index = lookup_index(&view, space_id, index_id)?;
                let count = unsafe { view.api.count(index, mode, &key) };
                if count < 0 {
                    Err(handle_tarantool_error())
                } else {
                    Ok(count as u64)
                }
            })
            .await
    }

    async fn drop(&mut self, self_: Resource<RawReadViewImpl>) -> wasmtime::Result<()> {
        let view = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            state.table.delete(self_)?
        };
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                drop(view);
            })
            .await;
        Ok(())
    }
}

impl read_view::HostIterator for SyncWasmStates {
    async fn next(
        &mut self,
        self_: Resource<RawReadViewIteratorImpl>,
    ) -> Result<Option<Vec<u8>>, BoxError> {
        let iterator = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let iterator = state
                .table
                .get_mut(&self_)
                .map_err(|err| unknown_error(format!("resource error: {err}")))?;
            SendPtr(iterator as *mut RawReadViewIteratorImpl)
        };
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let _region = BoxRegionGuard::new();
                let iterator = unsafe { &mut *iterator.get() };
                let inner = iterator
                    .inner
                    .as_mut()
                    .expect("live raw read-view iterator has an inner value");
                let (rc, data, size) =
                    unsafe { inner.view.api.iterator_next(inner.iterator.as_mut()) };
                if rc != 0 {
                    return Err(handle_tarantool_error());
                }
                copy_result(data, size)
            })
            .await
    }

    async fn drop(&mut self, self_: Resource<RawReadViewIteratorImpl>) -> wasmtime::Result<()> {
        let iterator = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            state.table.delete(self_)?
        };
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                drop(iterator);
            })
            .await;
        Ok(())
    }
}
