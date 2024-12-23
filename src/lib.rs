mod tarantool_wasm;
mod tx;
mod txffi;
mod bindings;
mod wasm_fiber;
use mlua::prelude::*;
use crate::tarantool_wasm::tt_wasm_load_module;
use crate::tarantool_wasm::tt_wasm_run_module;

fn load(_: &Lua, path: String) -> LuaResult<String> {
    match tt_wasm_load_module(path) {
        Ok(uid) => Ok(uid),
        Err(e) => Err(LuaError::runtime(e)),
    }
}

fn run(_: &Lua, module_uid: String) -> LuaResult<()> {
    tt_wasm_run_module(module_uid);
    Ok(())
}

#[mlua::lua_module]
fn my_module(lua: &Lua) -> LuaResult<LuaTable> {
    let exports = lua.create_table()?;
    exports.set("load", lua.create_function(load)?)?;
    exports.set("run", lua.create_function(run)?)?;
    Ok(exports)
}

