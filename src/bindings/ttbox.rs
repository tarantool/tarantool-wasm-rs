use crate::bindings::helpers::{debug_assert_tx_thread, validate_array};
use crate::bindings::tarantool::tarantool::ttbox;
use crate::bindings::tarantool::tarantool::types::{BoxError, BoxTuple, Index, Space};
use crate::ffi;
use crate::runtime::tarantool_wasm::SyncWasmStates;

use tarantool::index::{Index as TarantoolIndex, IndexId};
use tarantool::space::{Space as TarantoolSpace, SpaceId};
use tarantool::tuple::Tuple as TarantoolTuple;

use crate::bindings::error::map_tarantool_error;
use tarantool::error::TarantoolErrorCode;

impl ttbox::Host for SyncWasmStates {
    async fn schema_version(&mut self) -> i64 {
        self.executor()
            .execute(|| {
                debug_assert_tx_thread();
                unsafe { ffi::r#box::box_schema_version() as i64 }
            })
            .await
    }

    async fn space_by_name(&mut self, name: String) -> Result<Space, BoxError> {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                match TarantoolSpace::find(&name) {
                    Some(space) => Ok(Space {
                        id: space.id() as i32,
                    }),
                    None => Err(BoxError::new(
                        TarantoolErrorCode::NoSuchSpace as u32,
                        format!("Space '{name}' not found"),
                    )),
                }
            })
            .await
    }

    async fn index_by_name(&mut self, space: Space, name: String) -> Result<Index, BoxError> {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let spc = unsafe { TarantoolSpace::from_id_unchecked(space.id as SpaceId) };
                match spc.index(&name) {
                    Some(index) => Ok(Index {
                        id: index.id() as i32,
                        space_id: space.id,
                        index_base: 0,
                    }),
                    None => Err(BoxError::new(
                        TarantoolErrorCode::NoSuchIndexName as u32,
                        format!("Index '{}' not found in space {}", name, space.id),
                    )),
                }
            })
            .await
    }

    async fn insert(&mut self, space: Space, tup: Vec<u8>) -> Result<BoxTuple, BoxError> {
        validate_array(&tup)?;

        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let spc = unsafe { TarantoolSpace::from_id_unchecked(space.id as SpaceId) };

                let tuple = unsafe { TarantoolTuple::from_slice(&tup) };
                match spc.insert(&tuple) {
                    Ok(t) => Ok(BoxTuple {
                        ptr: t.as_ptr() as u64,
                    }),
                    Err(e) => Err(map_tarantool_error(e)),
                }
            })
            .await
    }

    async fn update(
        &mut self,
        index: Index,
        key: Vec<u8>,
        ops: Vec<u8>,
    ) -> Result<BoxTuple, BoxError> {
        validate_array(&key)?;
        validate_array(&ops)?;

        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let ind = unsafe {
                    TarantoolIndex::from_ids_unchecked(
                        index.space_id as SpaceId,
                        index.id as IndexId,
                    )
                };

                match unsafe { ind.update_raw(&key, &ops) } {
                    Ok(Some(t)) => Ok(BoxTuple {
                        ptr: t.as_ptr() as u64,
                    }),
                    Ok(None) => Err(BoxError::new(
                        TarantoolErrorCode::TupleNotFound as u32,
                        "Tuple not found or update returned nothing",
                    )),
                    Err(e) => Err(map_tarantool_error(e)),
                }
            })
            .await
    }

    async fn replace(&mut self, space: Space, tup: Vec<u8>) -> Result<BoxTuple, BoxError> {
        validate_array(&tup)?;

        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let spc = unsafe { TarantoolSpace::from_id_unchecked(space.id as SpaceId) };

                let tuple = unsafe { TarantoolTuple::from_slice(&tup) };
                match spc.replace(&tuple) {
                    Ok(t) => Ok(BoxTuple {
                        ptr: t.as_ptr() as u64,
                    }),
                    Err(e) => Err(map_tarantool_error(e)),
                }
            })
            .await
    }

    async fn upsert(&mut self, index: Index, key: Vec<u8>, ops: Vec<u8>) -> Result<(), BoxError> {
        validate_array(&key)?;
        validate_array(&ops)?;

        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let ind = unsafe {
                    TarantoolIndex::from_ids_unchecked(
                        index.space_id as SpaceId,
                        index.id as IndexId,
                    )
                };

                match unsafe { ind.upsert_raw(&key, &ops) } {
                    Ok(()) => Ok(()),
                    Err(e) => Err(map_tarantool_error(e)),
                }
            })
            .await
    }

    async fn delete(&mut self, index: Index, key: Vec<u8>) -> Result<BoxTuple, BoxError> {
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
                match ind.delete(&tuple) {
                    Ok(Some(t)) => Ok(BoxTuple {
                        ptr: t.as_ptr() as u64,
                    }),
                    Ok(None) => Err(BoxError::new(
                        TarantoolErrorCode::TupleNotFound as u32,
                        "Tuple not found or delete returned nothing",
                    )),
                    Err(e) => Err(map_tarantool_error(e)),
                }
            })
            .await
    }

    async fn truncate(&mut self, space: Space) -> Result<(), BoxError> {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let spc = unsafe { TarantoolSpace::from_id_unchecked(space.id as SpaceId) };

                match spc.truncate() {
                    Ok(()) => Ok(()),
                    Err(e) => Err(map_tarantool_error(e)),
                }
            })
            .await
    }
}
