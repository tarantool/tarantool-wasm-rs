use crate::bindings::docs::adder::index;
use crate::bindings::docs::adder::types::{BoxError, BoxTuple, Index, IteratorType};
use crate::tarantool_wasm::SyncWasmStates;
use crate::bindings::types::validate_msgpack_array;

use wasmtime_wasi::async_trait;
use tarantool::space::SpaceId;
use tarantool::tuple::Tuple as TarantoolTuple;
use tarantool::index::{Index as TarantoolIndex, IndexId};

#[async_trait]
impl index::Host for SyncWasmStates {
    async fn len(&mut self, index: Index) -> Result<i64, BoxError> {
        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(
                index.space_id as SpaceId,
                index.id as IndexId,
            )
        };

        match ind.len() {
            Ok(len) => Ok(len as i64),
            Err(e) => Err(BoxError::new(e.to_string(), "IndexError", 1))
        }
    }

    async fn bsize(&mut self, index: Index) -> Result<i64, BoxError> {
        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(
                index.space_id as SpaceId,
                index.id as IndexId,
            )
        };

        match ind.bsize() {
            Ok(bsize) => Ok(bsize as i64),
            Err(e) => Err(BoxError::new(e.to_string(), "IndexError", 1))
        }
    }

    async fn random(&mut self, index: Index, rnd: u32) -> Result<BoxTuple, BoxError> {
        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(
                index.space_id as SpaceId,
                index.id as IndexId,
            )
        };

        match ind.random(rnd) {
            Ok(Some(t)) => Ok(t.as_ptr() as BoxTuple),
            Ok(None) => Err(BoxError::new("Tuple not found or Index::random returned nothing", "TupleNotFound", 1)),
            Err(e) => Err(BoxError::new(e.to_string(), "IndexError", 1)),
        }
    }

    async fn get(&mut self, index: Index, key: Vec<u8>) -> Result<BoxTuple, BoxError> {
        if let Err(e) = validate_msgpack_array(&key) {
            return Err(BoxError::new(&e, "ValidationError", 1));
        }

        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(
                index.space_id as SpaceId,
                index.id as IndexId,
            )
        };

        let tuple = unsafe { TarantoolTuple::from_slice(&key) };

        match ind.get(&tuple) {
            Ok(Some(t)) => Ok(t.as_ptr() as BoxTuple),
            Ok(None) => Err(BoxError::new("Tuple not found or Index::get returned nothing", "TupleNotFound", 1)),
            Err(e) => Err(BoxError::new(e.to_string(), "IndexError", 1)),
        }
    }
    
    async fn min(&mut self, index: Index, key: Vec<u8>) -> Result<BoxTuple, BoxError> {
        if let Err(e) = validate_msgpack_array(&key) {
            return Err(BoxError::new(&e, "ValidationError", 1));
        }

        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(
                index.space_id as SpaceId,
                index.id as IndexId,
            )
        };

        let tuple = unsafe { TarantoolTuple::from_slice(&key) };

        match ind.min(&tuple) {
            Ok(Some(t)) => Ok(t.as_ptr() as BoxTuple),
            Ok(None) => Err(BoxError::new("Tuple not found or Index::min returned nothing", "TupleNotFound", 1)),
            Err(e) => Err(BoxError::new(e.to_string(), "IndexError", 1)),
        }
    }

    async fn max(&mut self, index: Index, key: Vec<u8>) -> Result<BoxTuple, BoxError> {
        if let Err(e) = validate_msgpack_array(&key) {
            return Err(BoxError::new(&e, "ValidationError", 1));
        }

        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(
                index.space_id as SpaceId,
                index.id as IndexId,
            )
        };

        let tuple = unsafe { TarantoolTuple::from_slice(&key) };

        match ind.max(&tuple) {
            Ok(Some(t)) => Ok(t.as_ptr() as BoxTuple),
            Ok(None) => Err(BoxError::new("Tuple not found or Index::max returned nothing", "TupleNotFound", 1)),
            Err(e) => Err(BoxError::new(e.to_string(), "IndexError", 1)),
        }
    }

    async fn count(&mut self, index: Index, iter_type: IteratorType, key: Vec<u8>) -> Result<i64, BoxError> {
        if let Err(e) = validate_msgpack_array(&key) {
            return Err(BoxError::new(&e, "ValidationError", 1));
        }

        let ind = unsafe {
            TarantoolIndex::from_ids_unchecked(
                index.space_id as SpaceId,
                index.id as IndexId,
            )
        };

        let iter = match iter_type {
            IteratorType::IterEq => tarantool::index::IteratorType::Eq,
            IteratorType::IterReq => tarantool::index::IteratorType::Req,
            IteratorType::IterAll => tarantool::index::IteratorType::All,
            IteratorType::IterLt => tarantool::index::IteratorType::LT,
            IteratorType::IterLe => tarantool::index::IteratorType::LE,
            IteratorType::IterGe => tarantool::index::IteratorType::GE,
            IteratorType::IterGt => tarantool::index::IteratorType::GT,
            IteratorType::IterBitsAllSet => tarantool::index::IteratorType::BitsAllSet,
            IteratorType::IterBitsAnySet => tarantool::index::IteratorType::BitsAnySet,
            IteratorType::IterBitsAllNotSet => tarantool::index::IteratorType::BitsAllNotSet,
            IteratorType::IterOverlaps => tarantool::index::IteratorType::Overlaps,
            IteratorType::IterNeighbor => tarantool::index::IteratorType::Neighbor,
            IteratorType::IterNp => tarantool::index::IteratorType::Neighbor,
            IteratorType::IterPp => tarantool::index::IteratorType::Neighbor,
        };

        let tuple = unsafe { TarantoolTuple::from_slice(&key) };
        match ind.count(iter, &tuple) {
            Ok(count) => Ok(count as i64),
            Err(e) => Err(BoxError::new(e.to_string(), "IndexError", 1)),
        }
    }
}
