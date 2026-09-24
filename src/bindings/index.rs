use crate::bindings::error::map_tarantool_error;
use crate::bindings::fallible_index_iterator::FallibleIndexIterator;
use crate::bindings::helpers::{debug_assert_tx_thread, unknown_error, validate_array};
use crate::bindings::tarantool::tarantool::index;
use crate::bindings::tarantool::tarantool::types::{BoxError, BoxTuple, Index, IteratorType};
use crate::bindings::types::convert_iterator_type;
use crate::bindings::IndexIteratorImpl;
use crate::runtime::tarantool_wasm::SyncWasmStates;
use crate::runtime::tx::run_on_tx_detached;

use tarantool::index::{Index as TarantoolIndex, IndexId};
use tarantool::space::SpaceId;
use tarantool::tuple::Tuple as TarantoolTuple;
use wasmtime::component::Resource;

fn native_index(index: Index) -> TarantoolIndex {
    unsafe { TarantoolIndex::from_ids_unchecked(index.space_id as SpaceId, index.id as IndexId) }
}

fn tuple_into_ptr(tuple: TarantoolTuple) -> u64 {
    let ptr = tuple.as_ptr() as u64;
    std::mem::forget(tuple);
    ptr
}

fn optional_tuple_ptr(tuple: Option<TarantoolTuple>) -> Option<u64> {
    tuple.map(tuple_into_ptr)
}

impl index::Host for SyncWasmStates {
    async fn tuple_count(&mut self, index: Index) -> Result<u64, BoxError> {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                native_index(index)
                    .len()
                    .map(|len| len as u64)
                    .map_err(map_tarantool_error)
            })
            .await
    }

    async fn memory_size(&mut self, index: Index) -> Result<u64, BoxError> {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                native_index(index)
                    .bsize()
                    .map(|size| size as u64)
                    .map_err(map_tarantool_error)
            })
            .await
    }

    async fn random(&mut self, index: Index, rnd: u32) -> Result<Option<BoxTuple>, BoxError> {
        let ptr = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                native_index(index)
                    .random(rnd)
                    .map(optional_tuple_ptr)
                    .map_err(map_tarantool_error)
            })
            .await?;
        ptr.map(|ptr| self.register_tuple(ptr)).transpose()
    }

    async fn get(&mut self, index: Index, key: Vec<u8>) -> Result<Option<BoxTuple>, BoxError> {
        validate_array(&key)?;
        let ptr = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                let key = unsafe { TarantoolTuple::from_slice(&key) };
                native_index(index)
                    .get(&key)
                    .map(optional_tuple_ptr)
                    .map_err(map_tarantool_error)
            })
            .await?;
        ptr.map(|ptr| self.register_tuple(ptr)).transpose()
    }

    async fn min(&mut self, index: Index, key: Vec<u8>) -> Result<Option<BoxTuple>, BoxError> {
        validate_array(&key)?;
        let ptr = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                let key = unsafe { TarantoolTuple::from_slice(&key) };
                native_index(index)
                    .min(&key)
                    .map(optional_tuple_ptr)
                    .map_err(map_tarantool_error)
            })
            .await?;
        ptr.map(|ptr| self.register_tuple(ptr)).transpose()
    }

    async fn max(&mut self, index: Index, key: Vec<u8>) -> Result<Option<BoxTuple>, BoxError> {
        validate_array(&key)?;
        let ptr = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                let key = unsafe { TarantoolTuple::from_slice(&key) };
                native_index(index)
                    .max(&key)
                    .map(optional_tuple_ptr)
                    .map_err(map_tarantool_error)
            })
            .await?;
        ptr.map(|ptr| self.register_tuple(ptr)).transpose()
    }

    async fn count(
        &mut self,
        index: Index,
        mode: IteratorType,
        key: Vec<u8>,
    ) -> Result<u64, BoxError> {
        validate_array(&key)?;
        let mode = convert_iterator_type(mode)?;
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let key = unsafe { TarantoolTuple::from_slice(&key) };
                native_index(index)
                    .count(mode, &key)
                    .map(|count| count as u64)
                    .map_err(map_tarantool_error)
            })
            .await
    }
}

// SAFETY: the iterator is only transported as an opaque value. Every
// operation, including destruction, is dispatched to the TX thread.
unsafe impl Send for IndexIteratorImpl {}

impl Drop for IndexIteratorImpl {
    fn drop(&mut self) {
        let Some(inner) = self.inner.take() else {
            return;
        };
        let pending = IndexIteratorImpl { inner: Some(inner) };
        run_on_tx_detached(move || {
            debug_assert_tx_thread();
            let mut pending = pending;
            drop(pending.inner.take());
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

impl index::HostIterator for SyncWasmStates {
    async fn new(
        &mut self,
        index: Index,
        mode: IteratorType,
        key: Vec<u8>,
    ) -> Result<Resource<IndexIteratorImpl>, BoxError> {
        validate_array(&key)?;
        let mode = convert_iterator_type(mode)?;
        let iter = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                FallibleIndexIterator::new(index, mode, key)
                    .map(|inner| IndexIteratorImpl { inner: Some(inner) })
            })
            .await?;

        let mut guard = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        guard
            .table
            .push(iter)
            .map_err(|err| unknown_error(format!("resource table error: {err}")))
    }

    async fn next(
        &mut self,
        self_: Resource<index::Iterator>,
    ) -> Result<Option<BoxTuple>, BoxError> {
        let iter_ptr = {
            let mut guard = self
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let iter = guard
                .table
                .get_mut(&self_)
                .map_err(|err| unknown_error(format!("resource error: {err}")))?;
            SendPtr(iter as *mut IndexIteratorImpl)
        };
        let ptr = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                // SAFETY: Wasmtime serializes host resource calls and the
                // resource table cannot be mutated while this call is pending.
                let iter = unsafe { &mut *iter_ptr.get() };
                iter.inner
                    .as_mut()
                    .expect("live iterator has an inner value")
                    .next()
                    .map(|tuple| tuple.map(tuple_into_ptr))
            })
            .await?;
        ptr.map(|ptr| self.register_tuple(ptr)).transpose()
    }

    async fn drop(&mut self, self_: Resource<index::Iterator>) -> Result<(), wasmtime::Error> {
        let iter = {
            let mut guard = self
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            guard.table.delete(self_)?
        };
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                drop(iter);
            })
            .await;
        Ok(())
    }
}
