use crate::bindings::helpers::{debug_assert_tx_thread, validate_array};
use crate::bindings::tarantool::tarantool::database;
use crate::bindings::tarantool::tarantool::types::{BoxError, BoxTuple, Index, Space};
use crate::ffi;
use crate::runtime::tarantool_wasm::SyncWasmStates;

use tarantool::index::{Index as TarantoolIndex, IndexId};
use tarantool::space::{Space as TarantoolSpace, SpaceId};
use tarantool::tuple::Tuple as TarantoolTuple;

use crate::bindings::error::{handle_tarantool_error, map_tarantool_error};

fn lookup_result<T>(found: Option<T>) -> Result<Option<T>, BoxError> {
    match found {
        Some(value) => Ok(Some(value)),
        None => match tarantool::error::BoxError::maybe_last() {
            Ok(()) => Ok(None),
            Err(_) => Err(handle_tarantool_error()),
        },
    }
}

fn tuple_into_ptr(tuple: TarantoolTuple) -> u64 {
    let ptr = tuple.as_ptr() as u64;
    std::mem::forget(tuple);
    ptr
}

impl database::Host for SyncWasmStates {
    async fn schema_version(&mut self) -> u32 {
        self.executor()
            .execute(|| {
                debug_assert_tx_thread();
                unsafe { ffi::r#box::box_schema_version() }
            })
            .await
    }

    async fn space_by_name(&mut self, name: String) -> Result<Option<Space>, BoxError> {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                tarantool::error::clear_error();
                let found = TarantoolSpace::find(&name).map(|space| Space { id: space.id() });
                lookup_result(found)
            })
            .await
    }

    async fn index_by_name(
        &mut self,
        space: Space,
        name: String,
    ) -> Result<Option<Index>, BoxError> {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let spc = unsafe { TarantoolSpace::from_id_unchecked(space.id as SpaceId) };
                tarantool::error::clear_error();
                let found = spc.index(&name).map(|index| Index {
                    id: index.id(),
                    space_id: space.id,
                });
                lookup_result(found)
            })
            .await
    }

    async fn insert(&mut self, space: Space, tup: Vec<u8>) -> Result<BoxTuple, BoxError> {
        validate_array(&tup)?;

        let ptr = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                let spc = unsafe { TarantoolSpace::from_id_unchecked(space.id as SpaceId) };

                let tuple = unsafe { TarantoolTuple::from_slice(&tup) };
                match spc.insert(&tuple) {
                    Ok(t) => Ok(tuple_into_ptr(t)),
                    Err(e) => Err(map_tarantool_error(e)),
                }
            })
            .await?;
        self.register_tuple(ptr)
    }

    async fn update(
        &mut self,
        index: Index,
        key: Vec<u8>,
        ops: Vec<u8>,
    ) -> Result<Option<BoxTuple>, BoxError> {
        validate_array(&key)?;
        validate_array(&ops)?;

        let ptr = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                let ind = unsafe {
                    TarantoolIndex::from_ids_unchecked(
                        index.space_id as SpaceId,
                        index.id as IndexId,
                    )
                };

                match unsafe { ind.update_raw(&key, &ops) } {
                    Ok(Some(t)) => Ok(Some(tuple_into_ptr(t))),
                    Ok(None) => Ok(None),
                    Err(e) => Err(map_tarantool_error(e)),
                }
            })
            .await?;
        ptr.map(|ptr| self.register_tuple(ptr)).transpose()
    }

    async fn replace(&mut self, space: Space, tup: Vec<u8>) -> Result<BoxTuple, BoxError> {
        validate_array(&tup)?;

        let ptr = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                let spc = unsafe { TarantoolSpace::from_id_unchecked(space.id as SpaceId) };

                let tuple = unsafe { TarantoolTuple::from_slice(&tup) };
                match spc.replace(&tuple) {
                    Ok(t) => Ok(tuple_into_ptr(t)),
                    Err(e) => Err(map_tarantool_error(e)),
                }
            })
            .await?;
        self.register_tuple(ptr)
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

    async fn delete(&mut self, index: Index, key: Vec<u8>) -> Result<Option<BoxTuple>, BoxError> {
        validate_array(&key)?;

        let ptr = self
            .executor()
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
                    Ok(Some(t)) => Ok(Some(tuple_into_ptr(t))),
                    Ok(None) => Ok(None),
                    Err(e) => Err(map_tarantool_error(e)),
                }
            })
            .await?;
        ptr.map(|ptr| self.register_tuple(ptr)).transpose()
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
