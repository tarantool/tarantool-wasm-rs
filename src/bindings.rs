use crate::tarantool_wasm::SyncWasmStates;
use std::thread;
use wasmtime::component::bindgen;
use wasmtime_wasi::async_trait;
use crate::bindings::docs::adder::tupler;
use crate::bindings::docs::adder::ttbox;
use crate::bindings::docs::adder::say;
use crate::bindings::docs::adder::error;
use crate::bindings::docs::adder::types::{Space, BoxError, Index, LogLevel};

use std::ffi::{CStr, CString};
use std::ptr;

pub type BoxTuple = u64;

use tarantool::space::{Space as TarantoolSpace, SpaceCreateOptions, SpaceEngineType, SpaceType};
use tarantool::error::TarantoolError;
use tarantool::tuple::Tuple;
use tarantool::log::{self, SayLevel};

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
    fn box_insert(space_id: u32, tuple: *const u8, tuple_end: *const u8, result: *mut *mut Tuple) -> i32;

    fn box_error_last() -> *const TarantoolError;
    fn box_error_type(error: *const TarantoolError) -> *const i8;
    fn box_error_code(error: *const TarantoolError) -> u32;
    fn box_error_message(error: *const TarantoolError) -> *const i8;
    fn box_error_clear();
    fn box_error_set(file: *const i8, line: u32, code: u32, format: *const i8, ...);
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

    async fn insert(&mut self, space: Space, tup: Vec<u8>) -> Result<BoxTuple, BoxError> {
        println!("RS | insert() called for space_id: {}", space.id);

        if tup.is_empty() {
            return Err(BoxError {
                message: "Tuple data is empty".to_string(),
                type_: "validation error".to_string(),
                code: 1,
                payload: None,
                file: None,
                line: None,
            });
        }

        let start = tup.as_ptr();
        let end = unsafe { start.add(tup.len()) };
        let mut result_tuple: *mut Tuple = ptr::null_mut();

        let rc = unsafe { box_insert(space.id as u32, start, end, &mut result_tuple) };

        if rc != 0 {
            let err_ptr = unsafe { box_error_last() };
            if !err_ptr.is_null() {
                let err = unsafe { &*err_ptr };
                println!("RS | Insert error: {}", err);
                return Err(BoxError {
                    message: err.to_string(),
                    type_: "tarantool error".to_string(),
                    code: err.error_code() as u32,
                    payload: None,
                    file: None,
                    line: None,
                });
            } else {
                return Err(BoxError {
                    message: "Unknown Tarantool error".to_string(),
                    type_: "tarantool error".to_string(),
                    code: 0,
                    payload: None,
                    file: None,
                    line: None,
                });
            }
        }

        if result_tuple.is_null() {
            return Err(BoxError {
                message: "Returned tuple is NULL".to_string(),
                type_: "tarantool error".to_string(),
                code: 0,
                payload: None,
                file: None,
                line: None,
            });
        }

        let tuple_id = result_tuple as u64;
        println!("RS | Insert successful, tuple_id: {}", tuple_id);
        Ok(tuple_id)
    }
}


impl From<LogLevel> for SayLevel {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::SFatal => SayLevel::Fatal,
            LogLevel::SSyserror => SayLevel::System,
            LogLevel::SError => SayLevel::Error,
            LogLevel::SCrit => SayLevel::Crit,
            LogLevel::SWarn => SayLevel::Warn,
            LogLevel::SInfo => SayLevel::Info,
            LogLevel::SVerbose => SayLevel::Verbose,
            LogLevel::SDebug => SayLevel::Debug,
        }
    }
}

#[async_trait]
impl say::Host for SyncWasmStates {
    async fn say(&mut self, level: LogLevel, msg: String) {
        let log_level: SayLevel = level.into();
        log::say(log_level, file!(), line!() as i32, None, &msg);
    }

    async fn say_error(&mut self, msg: String) {
        log::say(SayLevel::Error, file!(), line!() as i32, None, &msg);
    }

    async fn say_crit(&mut self, msg: String) {
        log::say(SayLevel::Crit, file!(), line!() as i32, None, &msg);
    }

    async fn say_warn(&mut self, msg: String) {
        log::say(SayLevel::Warn, file!(), line!() as i32, None, &msg);
    }

    async fn say_info(&mut self, msg: String) {
        log::say(SayLevel::Info, file!(), line!() as i32, None, &msg);
    }

    async fn say_verbose(&mut self, msg: String) {
        log::say(SayLevel::Verbose, file!(), line!() as i32, None, &msg);
    }

    async fn say_debug(&mut self, msg: String) {
        log::say(SayLevel::Debug, file!(), line!() as i32, None, &msg);
    }

    async fn say_syserror(&mut self, msg: String) {
        log::say(SayLevel::System, file!(), line!() as i32, None, &msg);
    }
}


#[async_trait]
impl error::Host for SyncWasmStates {
    async fn new(&mut self, message: String, type_: String, code: u32) -> BoxError {
        BoxError {
            message: message,
            type_: type_,
            code: code,
            payload: None,
            file: None,
            line: None,
        }
    }

    async fn set(&mut self, err: BoxError) {
        println!("RS | Setting error:");
        println!("RS | - Type: {}", err.type_);
        println!("RS | - Code: {}", err.code);
        println!("RS | - Message: {}", err.message);

        let file = CString::new("rust").unwrap();
        let format = CString::new(err.message.clone()).unwrap();

        unsafe {
            box_error_set(file.as_ptr(), 0, err.code, format.as_ptr());
        }

        println!("RS | Error set successfully");
    }

    async fn last(&mut self) -> Option<BoxError> {
        unsafe {
            let err_ptr = box_error_last();
            if err_ptr.is_null() {
                return None;
            }

            let message = CStr::from_ptr(box_error_message(err_ptr))
                .to_string_lossy()
                .into_owned();
            let type_ = CStr::from_ptr(box_error_type(err_ptr))
                .to_string_lossy()
                .into_owned();
            let code = box_error_code(err_ptr);

            Some(BoxError {
                    message: message,
                    type_: type_,
                    code: code,
                    payload: None,
                    file: None,
                    line: None,
            })
        }
    }

    async fn clear(&mut self) {
        unsafe {
            box_error_clear();
        }
    }

    async fn to_string(&mut self, err: BoxError) -> String {
        format!("[{}] {}: {}", err.code, err.type_, err.message)
    }
}








    // async fn update(&mut self, index: Index, key: Vec<u8>, ops: Vec<u8>) -> Result<BoxTuple, BoxError> {
    //     println!("Calling tarantool::box_::update for index id: {} key: {:?} ops: {:?}", index.id, key, ops);
    //     Ok(u64::MAX)
    // }

    // async fn replace(&mut self, space: Space,tup: &[u8],) -> Result<BoxTuple,BoxError>{
    // }

    // async fn truncate(&mut self, space: Space,) -> Result<(),BoxError>{
    // }

    // async fn delete(&mut self, index: Index,key: &[u8],) -> Result<BoxTuple,BoxError>{
    // }

    // async fn upsert(&mut self, index: Index,tup: &[u8],ops: &[u8],) -> Result<BoxTuple,BoxError>{
    // }