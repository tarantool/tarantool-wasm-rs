mod tarantool_wasm;
use mlua::prelude::*;
use crate::tarantool_wasm::tt_wasm_load_module;
use wasmtime::Module;

fn load(_: &Lua, path: String) -> LuaResult<Module> {
    tt_wasm_load_module(path.into())
}

#[mlua::lua_module]
fn my_module(lua: &Lua) -> LuaResult<LuaTable> {
    let exports = lua.create_table()?;
    exports.set("load", lua.create_function(load)?)?;
    Ok(exports)
}

