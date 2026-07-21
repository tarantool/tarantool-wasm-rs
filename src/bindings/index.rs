use crate::bindings::helpers::{debug_assert_tx_thread, validate_array};
use crate::bindings::tarantool::tarantool::index;
use crate::bindings::tarantool::tarantool::types::{BoxError, BoxTuple, Index, IteratorType};
use crate::bindings::types::convert_iterator_type;
use crate::bindings::IndexIteratorImpl;
use crate::runtime::tarantool_wasm::SyncWasmStates;
use crate::runtime::tx::run_on_tx_detached;

use tarantool::index::{Index as TarantoolIndex, IndexId};
use tarantool::space::SpaceId;
use tarantool::tuple::Tuple as TarantoolTuple;

use crate::bindings::error::map_tarantool_error;
use tarantool::error::TarantoolErrorCode;

use wasmtime::component::Resource;

impl index::Host for SyncWasmStates {
    async fn len(&mut self, index: Index) -> Result<i64, BoxError> {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let ind = unsafe {
                    TarantoolIndex::from_ids_unchecked(
                        index.space_id as SpaceId,
                        index.id as IndexId,
                    )
                };
                ind.len().map(|len| len as i64).map_err(map_tarantool_error)
            })
            .await
    }

    async fn bsize(&mut self, index: Index) -> Result<i64, BoxError> {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let ind = unsafe {
                    TarantoolIndex::from_ids_unchecked(
                        index.space_id as SpaceId,
                        index.id as IndexId,
                    )
                };
                ind.bsize()
                    .map(|bsize| bsize as i64)
                    .map_err(map_tarantool_error)
            })
            .await
    }

    async fn random(&mut self, index: Index, rnd: u32) -> Result<BoxTuple, BoxError> {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let ind = unsafe {
                    TarantoolIndex::from_ids_unchecked(
                        index.space_id as SpaceId,
                        index.id as IndexId,
                    )
                };
                match ind.random(rnd) {
                    Ok(Some(t)) => Ok(BoxTuple {
                        ptr: t.as_ptr() as u64,
                    }),
                    Ok(None) => Err(BoxError::new(
                        TarantoolErrorCode::TupleNotFound as u32,
                        "Tuple not found or Index::random returned nothing",
                    )),
                    Err(e) => Err(map_tarantool_error(e)),
                }
            })
            .await
    }

    async fn get(&mut self, index: Index, key: Vec<u8>) -> Result<BoxTuple, BoxError> {
        validate_array(&key)?;

        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let ind = unsafe {
                    TarantoolIndex::from_ids_unchecked(
                        index.space_id as SpaceId,
                        index.id as IndexId,
                    )
                };
                let tuple = unsafe { TarantoolTuple::from_slice(&key) };
                match ind.get(&tuple) {
                    Ok(Some(t)) => Ok(BoxTuple {
                        ptr: t.as_ptr() as u64,
                    }),
                    Ok(None) => Err(BoxError::new(
                        TarantoolErrorCode::TupleNotFound as u32,
                        "Tuple not found or Index::get returned nothing",
                    )),
                    Err(e) => Err(map_tarantool_error(e)),
                }
            })
            .await
    }

    async fn min(&mut self, index: Index, key: Vec<u8>) -> Result<BoxTuple, BoxError> {
        validate_array(&key)?;

        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let ind = unsafe {
                    TarantoolIndex::from_ids_unchecked(
                        index.space_id as SpaceId,
                        index.id as IndexId,
                    )
                };
                let tuple = unsafe { TarantoolTuple::from_slice(&key) };
                match ind.min(&tuple) {
                    Ok(Some(t)) => Ok(BoxTuple {
                        ptr: t.as_ptr() as u64,
                    }),
                    Ok(None) => Err(BoxError::new(
                        TarantoolErrorCode::TupleNotFound as u32,
                        "Tuple not found or Index::min returned nothing",
                    )),
                    Err(e) => Err(map_tarantool_error(e)),
                }
            })
            .await
    }

    async fn max(&mut self, index: Index, key: Vec<u8>) -> Result<BoxTuple, BoxError> {
        validate_array(&key)?;

        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let ind = unsafe {
                    TarantoolIndex::from_ids_unchecked(
                        index.space_id as SpaceId,
                        index.id as IndexId,
                    )
                };
                let tuple = unsafe { TarantoolTuple::from_slice(&key) };
                match ind.max(&tuple) {
                    Ok(Some(t)) => Ok(BoxTuple {
                        ptr: t.as_ptr() as u64,
                    }),
                    Ok(None) => Err(BoxError::new(
                        TarantoolErrorCode::TupleNotFound as u32,
                        "Tuple not found or Index::max returned nothing",
                    )),
                    Err(e) => Err(map_tarantool_error(e)),
                }
            })
            .await
    }

    async fn count(
        &mut self,
        index: Index,
        iter_type: IteratorType,
        key: Vec<u8>,
    ) -> Result<i64, BoxError> {
        validate_array(&key)?;

        let iter = convert_iterator_type(iter_type)?;
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let ind = unsafe {
                    TarantoolIndex::from_ids_unchecked(
                        index.space_id as SpaceId,
                        index.id as IndexId,
                    )
                };
                let tuple = unsafe { TarantoolTuple::from_slice(&key) };
                ind.count(iter, &tuple)
                    .map(|count| count as i64)
                    .map_err(map_tarantool_error)
            })
            .await
    }
}

// SAFETY: the iterator is only transported as an opaque value. Every operation,
// including destruction, is dispatched to and asserted to run on the TX thread.
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

// SAFETY: this wrapper only transports the pointer to the TX thread. Its
// pointee is never accessed on a worker thread.
unsafe impl<T> Send for SendPtr<T> {}

impl index::HostIndexIterator for SyncWasmStates {
    async fn new_iterator(
        &mut self,
        index: Index,
        iterator_type: IteratorType,
        key: Vec<u8>,
    ) -> Result<Resource<IndexIteratorImpl>, BoxError> {
        let native_iter_type = convert_iterator_type(iterator_type)?;
        let iter = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                let tuple = unsafe { tarantool::tuple::Tuple::from_slice(&key) };
                let ind = unsafe {
                    tarantool::index::Index::from_ids_unchecked(
                        index.space_id as tarantool::space::SpaceId,
                        index.id as tarantool::index::IndexId,
                    )
                };
                ind.select(native_iter_type, &tuple)
                    .map(|inner| IndexIteratorImpl { inner: Some(inner) })
                    .map_err(map_tarantool_error)
            })
            .await?;
        let mut guard = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let resource = guard.table.push(iter).map_err(|e| {
            BoxError::new(
                TarantoolErrorCode::Unknown as u32,
                format!("Resource table error: {e}"),
            )
        })?;
        Ok(resource)
    }

    async fn next(&mut self, self_: Resource<index::IndexIterator>) -> Result<BoxTuple, BoxError> {
        let iter_ptr = {
            let mut guard = self
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let iter = guard.table.get_mut(&self_).map_err(|e| {
                BoxError::new(
                    TarantoolErrorCode::Unknown as u32,
                    format!("Resource error: {e}"),
                )
            })?;
            SendPtr(iter as *mut IndexIteratorImpl)
        };
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                // SAFETY: host resource calls are serialized by Wasmtime and the
                // resource table cannot be mutated while this call is pending.
                let iter = unsafe { &mut *iter_ptr.get() };
                let inner = iter
                    .inner
                    .as_mut()
                    .expect("live iterator has an inner value");
                match inner.next() {
                    Some(t) => Ok(BoxTuple {
                        ptr: t.as_ptr() as u64,
                    }),
                    None => Err(BoxError::new(
                        TarantoolErrorCode::Unknown as u32,
                        "No more items",
                    )),
                }
            })
            .await
    }

    async fn drop(&mut self, self_: Resource<index::IndexIterator>) -> Result<(), wasmtime::Error> {
        let iter = {
            let mut guard: std::sync::MutexGuard<'_, crate::runtime::tarantool_wasm::WasmStates> =
                self.state
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
