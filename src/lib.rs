//! Lua bindings for executing WebAssembly modules inside Tarantool.
//!
//! The functions exposed from this crate are intended to be loaded from Lua
//! and provide high level access to the WASI-enabled Wasmtime runtime.

mod bindings;
mod ffi;
mod lua;
mod runtime;

use mlua::prelude::*;

/// Register the `wasm` Lua module with Tarantool.
#[mlua::lua_module(name = "wasm")]
fn my_module(lua: &Lua) -> LuaResult<LuaTable> {
    lua::api::create_module(lua)
}
