mod bindings;
mod ffi;
mod runtime;

use crate::runtime::convert::{from_rmpv, to_rmpv};
use crate::runtime::tarantool_wasm::{
    tt_wasm_call_function, tt_wasm_call_functions, tt_wasm_load_module, tt_wasm_run_module,
};
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
            Ok(true)
        }
        Err(err) => {
            Err(LuaError::runtime(err))
        }
    }
}

fn call(lua: &Lua, args: LuaMultiValue) -> LuaResult<Value> {
    use std::io::Cursor;
    let mut iter = args.into_iter();

    let module_uid = match iter.next() {
        Some(Value::String(s)) => {
            let s = s.to_str()?.to_string();
            s
        }
        _other => {
            return Err(mlua::Error::FromLuaConversionError {
                from: "Value",
                to: "String".to_string(),
                message: Some("expected module_uid as string".to_string()),
            });
        }
    };

    let func = match iter.next() {
        Some(Value::String(s)) => {
            let s = s.to_str()?.to_string();
            s
        }
        _other => {
            return Err(mlua::Error::FromLuaConversionError {
                from: "Value",
                to: "String".to_string(),
                message: Some("expected func name as string".to_string()),
            });
        }
    };

    let rest_args: Vec<Value> = iter.collect();

    let rmp_args = rest_args
        .iter()
        .map(to_rmpv)
        .collect::<Result<Vec<_>, _>>()?;

    let args_array = rmpv::Value::Array(rmp_args);
    let mut msgpack_args = Vec::new();
    rmpv::encode::write_value(&mut msgpack_args, &args_array)
        .map_err(|e| mlua::Error::external(format!("msgpack encode error: {e}")))?;

    let result_buf =
        tt_wasm_call_function(module_uid, func, msgpack_args).map_err(mlua::Error::external)?;

    let result_val =
        rmpv::decode::read_value(&mut Cursor::new(&result_buf)).map_err(mlua::Error::external)?;

    let lua_val = from_rmpv(lua, &result_val)?;
    Ok(lua_val)
}

fn batch(lua: &Lua, args: LuaMultiValue) -> LuaResult<Value> {
    use std::io::Cursor;

    let mut iter = args.into_iter();

    let module_uid = match iter.next() {
        Some(Value::String(s)) => s.to_str()?.to_string(),
        _ => {
            return Err(mlua::Error::FromLuaConversionError {
                from: "Value",
                to: "String".to_string(),
                message: Some("expected module_uid as string".to_string()),
            })
        }
    };

    let calls_tbl = match iter.next() {
        Some(Value::Table(t)) => t,
        _ => {
            return Err(mlua::Error::FromLuaConversionError {
                from: "Value",
                to: "table".to_string(),
                message: Some("expected calls table".to_string()),
            });
        }
    };

    let mut calls = Vec::new();
    for pair in calls_tbl.sequence_values::<LuaValue>() {
        let val = pair?;
        let call_tbl = match val {
            LuaValue::Table(t) => t,
            _ => {
                return Err(mlua::Error::FromLuaConversionError {
                    from: "Value",
                    to: "table".to_string(),
                    message: Some("each call must be table".to_string()),
                });
            }
        };

        let func: String = match call_tbl.raw_get(1)? {
            LuaValue::String(s) => s.to_str()?.to_string(),
            _ => {
                return Err(mlua::Error::FromLuaConversionError {
                    from: "Value",
                    to: "string".to_string(),
                    message: Some("call entry first element must be func name".to_string()),
                });
            }
        };

        let args_vals: Vec<LuaValue> = {
            let mut vec = Vec::new();
            for idx in 2..=call_tbl.raw_len() {
                vec.push(call_tbl.raw_get(idx)?);
            }
            vec
        };

        let rmp_args = args_vals
            .iter()
            .map(to_rmpv)
            .collect::<Result<Vec<_>, _>>()?;

        let args_array = rmpv::Value::Array(rmp_args);
        let mut msgpack_args = Vec::new();
        rmpv::encode::write_value(&mut msgpack_args, &args_array)
            .map_err(|e| mlua::Error::external(format!("msgpack encode error: {e}")))?;

        calls.push((func, msgpack_args));
    }

    let result_bufs = tt_wasm_call_functions(module_uid, calls).map_err(mlua::Error::external)?;

    let results_tbl = lua.create_table()?;
    for (i, buf) in result_bufs.into_iter().enumerate() {
        let result_val =
            rmpv::decode::read_value(&mut Cursor::new(&buf)).map_err(mlua::Error::external)?;
        let lua_val = from_rmpv(lua, &result_val)?;
        results_tbl.set(i + 1, lua_val)?;
    }

    Ok(LuaValue::Table(results_tbl))
}

#[mlua::lua_module(name = "wasm")]
fn my_module(lua: &Lua) -> LuaResult<LuaTable> {
    let exports = lua.create_table()?;
    exports.set("load", lua.create_function(load)?)?;
    exports.set("run", lua.create_function(run)?)?;
    exports.set("join", lua.create_function(join)?)?;
    exports.set("call", lua.create_function(call)?)?;
    exports.set("batch", lua.create_function(batch)?)?;
    Ok(exports)
}
