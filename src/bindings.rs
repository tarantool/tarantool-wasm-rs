use crate::tarantool_wasm::SyncWasmStates;
use std::thread;
use wasmtime::component::bindgen;
use wasmtime_wasi::async_trait;
use crate::bindings::docs::adder::tupler;
use crate::bindings::docs::adder::ttbox;
use crate::bindings::docs::adder::types::{Space, BoxError, Index};

use std::ffi::CString;
use std::ptr;

// pub type BoxTuple = u64;

use tarantool::space::{Space as TarantoolSpace, SpaceCreateOptions, SpaceEngineType, SpaceType};
use tarantool::tuple::Tuple;
use tarantool::error::Error as TarantoolError;

bindgen!({
    path: "adder.wit",
    world: "adder",
    async: true,
    with: {
        "docs:adder/tupler/space": TtSpace,
    }
});

extern "C" {
    fn box_schema_version() -> u32;
    fn box_index_id_by_name(space_id: u32, name: *const u8, len: u32) -> u32;
}

pub struct TtSpace {
    space_id: u32
}

#[async_trait]
impl tupler::Host for SyncWasmStates {
    async fn get_tuple(&mut self, name: String, value: Vec<u8>) -> tupler::TtTuple {
        let current_thread = thread::current();
        println!("Current thread name: {:?}", current_thread.name());
        self.yield_now().await;
        let current_thread = thread::current();
        println!("Current thread name: {:?}", current_thread.name());

        tupler::TtTuple {name, value, status: true}
    }
}

#[async_trait]
impl tupler::HostSpace for SyncWasmStates {
    async fn new(&mut self, name: String) -> wasmtime::component::Resource<TtSpace> {
        let space_id = if name == "test" { 512 }  else  { 0 };
        self.0.lock().unwrap().table.push(TtSpace{space_id}).unwrap()
    }

    async fn insert(&mut self, _self_: wasmtime::component::Resource<TtSpace>, tup: tupler::TtTuple) -> Result<tupler::TtTuple, String> {

        println!("TX");
        let current_thread = thread::current();
        println!("Current thread name: {:?}", current_thread.name());
        self.yield_now().await;
        let current_thread = thread::current();
        println!("Current thread name: {:?}", current_thread.name());
        Ok(tup)
    }

    async fn drop(&mut self, self_: wasmtime::component::Resource<TtSpace>) -> Result<(), wasmtime::Error> {
        self.0.lock().unwrap().table.delete(self_);
        Ok(())
    }
}

fn convert_error<E: std::fmt::Display>(e: E) -> BoxError {
    BoxError {
        message: e.to_string(),
        type_: "tarantool error".to_string(),
        code: 0,
        payload: None,
        file: None,
        line: None,
    }
}

#[async_trait]
impl ttbox::Host for SyncWasmStates {
    async fn schema_version(&mut self) -> i64 {
        println!("RS | Executing adder::box::schema_version");
        unsafe { box_schema_version() as i64 }
    }

    async fn space_by_name(&mut self, name: String) -> Result<Space, BoxError> {
        println!("RS | Calling tarantool::box_::space_by_name with name: {}", name);
        
        match TarantoolSpace::find(&name) {
            Some(space) => {
                let space_id = space.id() as i32;
                println!("RS | Found space: name = '{}', id = {}", name, space_id);
                Ok(Space { id: space_id })
            }
            None => {
                println!("RS |Space '{}' not found", name);
                Err(convert_error(format!("Space '{}' not found", name)))
            }
        }
    }

    async fn index_by_name(&mut self, space: Space, name: String) -> Result<Index, BoxError> {
        println!("RS | index_by_name('{}') called for space_id: {}", name, space.id);

        let c_name = match CString::new(name.clone()) {
            Ok(cstr) => cstr,
            Err(_) => {
                return Err(BoxError {
                    message: "CString conversion failed".to_string(),
                    type_: "internal error".to_string(),
                    code: 1,
                    payload: None,
                    file: None,
                    line: None,
                });
            }
        };

        let index_id = unsafe {
            box_index_id_by_name(space.id as u32, c_name.as_ptr() as *const u8, name.len() as u32)
        };

        if index_id == u32::MAX {
            println!("RS | Index '{}' not found in space_id {}", name, space.id);
            return Err(BoxError {
                message: format!("Index '{}' not found in space_id {}", name, space.id),
                type_: "tarantool error".to_string(),
                code: 0,
                payload: None,
                file: None,
                line: None,
            });
        }

        println!("RS | Found index '{}' with id {} in space_id {}", name, index_id, space.id);

        Ok(Index {
            id: index_id as i32,
            space_id: space.id,
            index_base: 1,
        })
    }

    // async fn update(&mut self, index: Index, key: Vec<u8>, ops: Vec<u8>) -> Result<BoxTuple, BoxError> {
    //     println!("Calling tarantool::box_::update for index id: {} key: {:?} ops: {:?}", index.id, key, ops);
    //     Ok(u64::MAX)
    // }
}
    // async fn index_by_name(&mut self, name: &str,index_base: i32,) -> Result<Index,BoxError>{
    // }

    // async fn insert(&mut self, space: Space,tup: &[u8],) -> Result<BoxTuple,BoxError>{
    // }

    // async fn replace(&mut self, space: Space,tup: &[u8],) -> Result<BoxTuple,BoxError>{
    // }

    // async fn truncate(&mut self, space: Space,) -> Result<(),BoxError>{
    // }

    // async fn delete(&mut self, index: Index,key: &[u8],) -> Result<BoxTuple,BoxError>{
    // }

    // async fn upsert(&mut self, index: Index,tup: &[u8],ops: &[u8],) -> Result<BoxTuple,BoxError>{
    // }