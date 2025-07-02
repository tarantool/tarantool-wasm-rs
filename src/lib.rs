mod bindings;
mod executor;
mod ffi;
mod tarantool_wasm;
mod tx;
mod wasm_fiber;
use crate::tarantool_wasm::tt_wasm_load_module;
use crate::tarantool_wasm::tt_wasm_run_module;
use anyhow::Result;
use mlua::prelude::*;
use mlua::Value;
use std::thread::JoinHandle;
use tarantool::coio::coio_call;

struct WasmJoinHandle(JoinHandle<Result<()>>);

impl LuaUserData for WasmJoinHandle {}
impl FromLua for WasmJoinHandle {
    fn from_lua(value: Value, _lua: &'_ Lua) -> std::result::Result<WasmJoinHandle, mlua::Error> {
        value
            .as_userdata()
            .ok_or(LuaError::runtime("argument is not a join handle"))?
            .take()
    }
}

fn load(_: &Lua, path: String) -> LuaResult<String> {
    match tt_wasm_load_module(path) {
        Ok(uid) => Ok(uid),
        Err(e) => Err(LuaError::runtime(e)),
    }
}

fn run(_: &Lua, module_uid: String) -> LuaResult<WasmJoinHandle> {
    match tt_wasm_run_module(module_uid) {
        Ok(handle) => Ok(WasmJoinHandle(handle)),
        Err(e) => Err(mlua::Error::runtime(e)),
    }
}

fn join(_: &Lua, handle: WasmJoinHandle) -> LuaResult<bool> {
    let mut result: Result<()> = Ok(());
    coio_call(
        &mut |handle: Box<JoinHandle<Result<()>>>| {
            result = handle.join().unwrap();
            0
        },
        handle.0,
    );
    match result {
        Ok(()) => {
            return Ok(true);
        }
        Err(err) => {
            return Err(LuaError::runtime(err));
        }
    }
}

#[mlua::lua_module(name = "wasm")]
fn my_module(lua: &Lua) -> LuaResult<LuaTable> {
    let exports = lua.create_table()?;
    exports.set("load", lua.create_function(load)?)?;
    exports.set("run", lua.create_function(run)?)?;
    exports.set("join", lua.create_function(join)?)?;
    Ok(exports)
}
