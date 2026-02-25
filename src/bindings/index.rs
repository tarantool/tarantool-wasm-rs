use crate::bindings::helpers::validate_array;
use crate::bindings::tarantool::tarantool::index;
use crate::bindings::tarantool::tarantool::types::{BoxError, BoxTuple, Index, IteratorType};
use crate::bindings::types::convert_iterator_type;
use crate::bindings::IndexIteratorImpl;
use crate::runtime::tarantool_wasm::SyncWasmStates;

use tarantool::index::{Index as TarantoolIndex, IndexId};
use tarantool::space::SpaceId;
use tarantool::tuple::Tuple as TarantoolTuple;

use crate::bindings::error::map_tarantool_error;
use tarantool::error::TarantoolErrorCode;

use wasmtime::component::Resource;

impl index::Host for SyncWasmStates {
    async fn len(&mut self, index: Index) -> Result<i64, BoxError> {
        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(index.space_id as SpaceId, index.id as IndexId)
        };

        match ind.len() {
            Ok(len) => Ok(len as i64),
            Err(e) => Err(map_tarantool_error(e)),
        }
    }

    async fn bsize(&mut self, index: Index) -> Result<i64, BoxError> {
        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(index.space_id as SpaceId, index.id as IndexId)
        };

        match ind.bsize() {
            Ok(bsize) => Ok(bsize as i64),
            Err(e) => Err(map_tarantool_error(e)),
        }
    }

    async fn random(&mut self, index: Index, rnd: u32) -> Result<BoxTuple, BoxError> {
        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(index.space_id as SpaceId, index.id as IndexId)
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
    }

    async fn get(&mut self, index: Index, key: Vec<u8>) -> Result<BoxTuple, BoxError> {
        validate_array(&key)?;

        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(index.space_id as SpaceId, index.id as IndexId)
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
    }

    async fn min(&mut self, index: Index, key: Vec<u8>) -> Result<BoxTuple, BoxError> {
        validate_array(&key)?;

        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(index.space_id as SpaceId, index.id as IndexId)
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
    }

    async fn max(&mut self, index: Index, key: Vec<u8>) -> Result<BoxTuple, BoxError> {
        validate_array(&key)?;

        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(index.space_id as SpaceId, index.id as IndexId)
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
    }

    async fn count(
        &mut self,
        index: Index,
        iter_type: IteratorType,
        key: Vec<u8>,
    ) -> Result<i64, BoxError> {
        validate_array(&key)?;

        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(index.space_id as SpaceId, index.id as IndexId)
        };

        let iter = convert_iterator_type(iter_type)?;
        let tuple = unsafe { TarantoolTuple::from_slice(&key) };

        match ind.count(iter, &tuple) {
            Ok(count) => Ok(count as i64),
            Err(e) => Err(map_tarantool_error(e)),
        }
    }
}

unsafe impl Send for IndexIteratorImpl {}
unsafe impl Sync for IndexIteratorImpl {}

impl index::HostIndexIterator for SyncWasmStates {
    async fn new_iterator(
        &mut self,
        index: Index,
        iterator_type: IteratorType,
        key: Vec<u8>,
    ) -> Result<Resource<IndexIteratorImpl>, BoxError> {
        let native_iter_type = convert_iterator_type(iterator_type)?;
        let tuple = unsafe { tarantool::tuple::Tuple::from_slice(&key) };

        let ind = unsafe {
            tarantool::index::Index::from_ids_unchecked(
                index.space_id as tarantool::space::SpaceId,
                index.id as tarantool::index::IndexId,
            )
        };

        let iter = ind
            .select(native_iter_type, &tuple)
            .map_err(map_tarantool_error)?;
        let mut guard = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let resource = guard
            .table
            .push(IndexIteratorImpl { inner: iter })
            .map_err(|e| {
                BoxError::new(
                    TarantoolErrorCode::Unknown as u32,
                    format!("Resource table error: {e}"),
                )
            })?;
        Ok(resource)
    }

    async fn next(&mut self, self_: Resource<index::IndexIterator>) -> Result<BoxTuple, BoxError> {
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

        match iter.inner.next() {
            Some(t) => Ok(BoxTuple {
                ptr: t.as_ptr() as u64,
            }),
            None => Err(BoxError::new(
                TarantoolErrorCode::Unknown as u32,
                "No more items",
            )),
        }
    }

    async fn drop(&mut self, self_: Resource<index::IndexIterator>) -> Result<(), wasmtime::Error> {
        let mut guard: std::sync::MutexGuard<'_, crate::runtime::tarantool_wasm::WasmStates> = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        guard.table.delete(self_)?;
        Ok(())
    }
}
