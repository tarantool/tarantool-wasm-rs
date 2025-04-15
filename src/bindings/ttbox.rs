use crate::bindings::tarantool::tarantool::ttbox;
use crate::bindings::tarantool::tarantool::types::{BoxError, Index, Space};
use crate::tarantool_wasm::SyncWasmStates;
use crate::ffi as ffi;
use crate::bindings::types::validate_msgpack_array;

use wasmtime_wasi::async_trait;
use tarantool::space::{Space as TarantoolSpace, SpaceId};
use tarantool::tuple::Tuple as TarantoolTuple;
use tarantool::index::{Index as TarantoolIndex, IndexId};

pub type BoxTuple = u64;

#[async_trait]
impl ttbox::Host for SyncWasmStates {
    async fn schema_version(&mut self) -> i64 {
        unsafe {
            ffi::r#box::box_schema_version() as i64
        }
    }

    async fn space_by_name(&mut self, name: String) -> Result<Space, BoxError> {
        match TarantoolSpace::find(&name) {
            Some(space) => Ok(Space{id: space.id() as i32}),
            None => Err(BoxError::new(&format!("Space '{}' not found", name), "SpaceNotFound", 1)),
        }
    }

    async fn index_by_name(&mut self, space: Space, name: String) -> Result<Index, BoxError> {
        let spc = unsafe { TarantoolSpace::from_id_unchecked(space.id as SpaceId) };

        match spc.index(&name) {
            Some(index) => Ok(Index {
                id: index.id() as i32,
                space_id: space.id,
                index_base: 0,
            }),
            None => Err(BoxError::new(
                &format!("Index '{}' not found in space {}", name, space.id),
                "IndexNotFound",
                1,
            )),
        }
    }

    async fn insert(&mut self, space: Space, tup: Vec<u8>) -> Result<BoxTuple, BoxError> {        
        if let Err(e) = validate_msgpack_array(&tup) {
            return Err(BoxError::new(&e, "ValidationError", 1));
        }

        let tup_copy = tup.clone();
        let space_copy = space.clone();

        self.executor().execute(move || {
            let spc = unsafe {
                TarantoolSpace::from_id_unchecked(space_copy.id as SpaceId)
            };

            let tuple = unsafe { TarantoolTuple::from_slice(&tup_copy) };
            match spc.insert(&tuple) {
                Ok(t) => return Ok(t.as_ptr() as BoxTuple),
                Err(e) => return Err(BoxError::new(e.to_string(), "InsertError", 1))
            }
        }).await
    }

    async fn update(&mut self, index: Index, key: Vec<u8>, ops: Vec<u8>) -> Result<BoxTuple, BoxError> {
        if let Err(e) = validate_msgpack_array(&key) {
            return Err(BoxError::new(&e, "ValidationError", 1));
        }
        if let Err(e) = validate_msgpack_array(&ops) {
            return Err(BoxError::new(&e, "ValidationError", 1));
        }

        let index_copy = index.clone();
        let key_copy = key.clone();
        let ops_copy = ops.clone();

        self.executor().execute(move || {
            let ind = unsafe {
                TarantoolIndex::from_ids_unchecked(
                    index_copy.space_id as SpaceId,
                    index_copy.id as IndexId,
                )
            };
    
            match unsafe { ind.update_raw(&key_copy, &ops_copy) } {
                Ok(Some(t)) => Ok(t.as_ptr() as BoxTuple),
                Ok(None) => Err(BoxError::new("Tuple not found or update returned nothing", "TupleNotFound", 1)),
                Err(e) => Err(BoxError::new(e.to_string(), "UpdateError", 1)),
            }
        }).await
    }

    async fn replace(&mut self, space: Space, tup: Vec<u8>) -> Result<BoxTuple, BoxError> {
        if let Err(e) = validate_msgpack_array(&tup) {
            return Err(BoxError::new(&e, "ValidationError", 1));
        }

        let tup_copy = tup.clone();
        let space_copy = space.clone();

        self.executor().execute(move || {
            let spc = unsafe {
                TarantoolSpace::from_id_unchecked(space_copy.id as SpaceId)
            };

            let tuple = unsafe { TarantoolTuple::from_slice(&tup_copy) };
            match spc.replace(&tuple) {
                Ok(t) => return Ok(t.as_ptr() as BoxTuple),
                Err(e) => return Err(BoxError::new(e.to_string(), "ReplaceError", 1))
            }
        }).await
    }

    async fn upsert(&mut self, index: Index, key: Vec<u8>, ops: Vec<u8>) -> Result<(), BoxError> {
        if let Err(e) = validate_msgpack_array(&key) {
            return Err(BoxError::new(&e, "ValidationError", 1));
        }
        if let Err(e) = validate_msgpack_array(&ops) {
            return Err(BoxError::new(&e, "ValidationError", 1));
        }

        let index_copy = index.clone();
        let key_copy = key.clone();
        let ops_copy = ops.clone();

        self.executor().execute(move || {
            let ind = unsafe {
                TarantoolIndex::from_ids_unchecked(
                    index_copy.space_id as SpaceId,
                    index_copy.id as IndexId,
                )
            };
    
            match unsafe { ind.upsert_raw(&key_copy, &ops_copy) } {
                Ok(()) => Ok(()),
                Err(e) => Err(BoxError::new(e.to_string(), "UpsertError", 1)),
            }
        }).await
    }

    async fn delete(&mut self, index: Index, key: Vec<u8>) -> Result<BoxTuple, BoxError> {
        if let Err(e) = validate_msgpack_array(&key) {
            return Err(BoxError::new(&e, "ValidationError", 1));
        }

        let index_copy = index.clone();
        let key_copy = key.clone();

        self.executor().execute(move || {
            let ind = unsafe {
                TarantoolIndex::from_ids_unchecked(
                    index_copy.space_id as SpaceId,
                    index_copy.id as IndexId,
                )
            };

            let tuple = unsafe { TarantoolTuple::from_slice(&key_copy) };
            match ind.delete(&tuple) {
                Ok(Some(t)) => Ok(t.as_ptr() as BoxTuple),
                Ok(None) => Err(BoxError::new("Tuple not found or delete returned nothing", "TupleNotFound", 1)),
                Err(e) => Err(BoxError::new(e.to_string(), "DeleteError", 1)),
            }
        }).await
    }

    async fn truncate(&mut self, space: Space) -> Result<(), BoxError> {
        let space_copy = space.clone();

        self.executor().execute(move || {
            let spc = unsafe {
                TarantoolSpace::from_id_unchecked(space_copy.id as SpaceId)
            };

            match spc.truncate() {
                Ok(()) => Ok(()),
                Err(e) => return Err(BoxError::new(e.to_string(), "InsertError", 1))
            }
        }).await
    } 
}
