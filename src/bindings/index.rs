use crate::bindings::tarantool::tarantool::index;
use crate::bindings::tarantool::tarantool::types::{BoxError, BoxTuple, Index, IteratorType};
use crate::bindings::types::{convert_iterator_type, validate_msgpack_array};
use crate::runtime::tarantool_wasm::SyncWasmStates;

use tarantool::index::{Index as TarantoolIndex, IndexId};
use tarantool::space::SpaceId;
use tarantool::tuple::Tuple as TarantoolTuple;

impl index::Host for SyncWasmStates {
    async fn len(&mut self, index: Index) -> Result<i64, BoxError> {
        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(index.space_id as SpaceId, index.id as IndexId)
        };

        match ind.len() {
            Ok(len) => Ok(len as i64),
            Err(e) => Err(BoxError::new(e.to_string(), "IndexError", 1)),
        }
    }

    async fn bsize(&mut self, index: Index) -> Result<i64, BoxError> {
        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(index.space_id as SpaceId, index.id as IndexId)
        };

        match ind.bsize() {
            Ok(bsize) => Ok(bsize as i64),
            Err(e) => Err(BoxError::new(e.to_string(), "IndexError", 1)),
        }
    }

    async fn random(&mut self, index: Index, rnd: u32) -> Result<BoxTuple, BoxError> {
        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(index.space_id as SpaceId, index.id as IndexId)
        };

        match ind.random(rnd) {
            Ok(Some(t)) => Ok(t.as_ptr() as BoxTuple),
            Ok(None) => Err(BoxError::new(
                "Tuple not found or Index::random returned nothing",
                "TupleNotFound",
                1,
            )),
            Err(e) => Err(BoxError::new(e.to_string(), "IndexError", 1)),
        }
    }

    async fn get(&mut self, index: Index, key: Vec<u8>) -> Result<BoxTuple, BoxError> {
        if let Err(e) = validate_msgpack_array(&key) {
            return Err(BoxError::new(&e, "ValidationError", 1));
        }

        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(index.space_id as SpaceId, index.id as IndexId)
        };

        let tuple = unsafe { TarantoolTuple::from_slice(&key) };

        match ind.get(&tuple) {
            Ok(Some(t)) => Ok(t.as_ptr() as BoxTuple),
            Ok(None) => Err(BoxError::new(
                "Tuple not found or Index::get returned nothing",
                "TupleNotFound",
                1,
            )),
            Err(e) => Err(BoxError::new(e.to_string(), "IndexError", 1)),
        }
    }

    async fn min(&mut self, index: Index, key: Vec<u8>) -> Result<BoxTuple, BoxError> {
        if let Err(e) = validate_msgpack_array(&key) {
            return Err(BoxError::new(&e, "ValidationError", 1));
        }

        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(index.space_id as SpaceId, index.id as IndexId)
        };

        let tuple = unsafe { TarantoolTuple::from_slice(&key) };

        match ind.min(&tuple) {
            Ok(Some(t)) => Ok(t.as_ptr() as BoxTuple),
            Ok(None) => Err(BoxError::new(
                "Tuple not found or Index::min returned nothing",
                "TupleNotFound",
                1,
            )),
            Err(e) => Err(BoxError::new(e.to_string(), "IndexError", 1)),
        }
    }

    async fn max(&mut self, index: Index, key: Vec<u8>) -> Result<BoxTuple, BoxError> {
        if let Err(e) = validate_msgpack_array(&key) {
            return Err(BoxError::new(&e, "ValidationError", 1));
        }

        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(index.space_id as SpaceId, index.id as IndexId)
        };

        let tuple = unsafe { TarantoolTuple::from_slice(&key) };

        match ind.max(&tuple) {
            Ok(Some(t)) => Ok(t.as_ptr() as BoxTuple),
            Ok(None) => Err(BoxError::new(
                "Tuple not found or Index::max returned nothing",
                "TupleNotFound",
                1,
            )),
            Err(e) => Err(BoxError::new(e.to_string(), "IndexError", 1)),
        }
    }

    async fn count(
        &mut self,
        index: Index,
        iter_type: IteratorType,
        key: Vec<u8>,
    ) -> Result<i64, BoxError> {
        if let Err(e) = validate_msgpack_array(&key) {
            return Err(BoxError::new(&e, "ValidationError", 1));
        }

        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(index.space_id as SpaceId, index.id as IndexId)
        };

        let iter = convert_iterator_type(iter_type)?;
        let tuple = unsafe { TarantoolTuple::from_slice(&key) };

        match ind.count(iter, &tuple) {
            Ok(count) => Ok(count as i64),
            Err(e) => Err(BoxError::new(e.to_string(), "IndexError", 1)),
        }
    }
}
