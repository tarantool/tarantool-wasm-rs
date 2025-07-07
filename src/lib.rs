mod bindings;
mod executor;
mod ffi;
mod tarantool_wasm;
mod tx;
mod wasm_fiber;
use crate::tarantool_wasm::tt_wasm_call_function;
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

fn to_rmpv(val: &mlua::Value) -> Result<rmpv::Value, mlua::Error> {
    use mlua::Value::*;
    Ok(match val {
        Nil => rmpv::Value::Nil,
        Boolean(b) => rmpv::Value::Boolean(*b),
        Integer(i) => rmpv::Value::Integer((*i).into()),
        Number(n) => rmpv::Value::F64(*n),
        String(s) => {
            let str_val = s.to_str()?;
            rmpv::Value::String(rmpv::Utf8String::from(str_val.to_string()))
        }
        Table(t) => {
            // Try array-style first, fallback to map
            let is_array = t.clone().pairs::<mlua::Value, mlua::Value>().all(|p| {
                p.map(|(k, _)| matches!(k, mlua::Value::Integer(_)))
                    .unwrap_or(false)
            });

            if is_array {
                let mut vec = Vec::new();
                for pair in t.clone().sequence_values::<mlua::Value>() {
                    let v = pair?;
                    vec.push(to_rmpv(&v)?);
                }
                rmpv::Value::Array(vec)
            } else {
                let mut map = Vec::new();
                for pair in t.clone().pairs::<mlua::Value, mlua::Value>() {
                    let (k, v) = pair?;
                    map.push((to_rmpv(&k)?, to_rmpv(&v)?));
                }
                rmpv::Value::Map(map)
            }
        }
        _ => return Err(mlua::Error::external("unsupported Lua type")),
    })
}

fn from_rmpv(lua: &mlua::Lua, val: &rmpv::Value) -> mlua::Result<mlua::Value> {
    use mlua::Value as LV;
    use rmpv::Value::*;

    Ok(match val {
        Nil => LV::Nil,
        Boolean(b) => LV::Boolean(*b),
        Integer(i) => LV::Integer(i.as_i64().unwrap_or(0)),
        F64(f) => LV::Number(*f),
        String(s) => LV::String(lua.create_string(s.as_str().unwrap_or(""))?),
        Array(arr) => {
            let seq = arr
                .iter()
                .map(|v| from_rmpv(lua, v))
                .collect::<mlua::Result<Vec<_>>>()?;
            LV::Table(lua.create_sequence_from(seq)?)
        }
        Map(map) => {
            let tbl = lua.create_table()?;
            for (k, v) in map {
                tbl.set(from_rmpv(lua, k)?, from_rmpv(lua, v)?)?;
            }
            LV::Table(tbl)
        }
        _ => return Err(mlua::Error::external("unsupported return type")),
    })
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

#[mlua::lua_module(name = "wasm")]
fn my_module(lua: &Lua) -> LuaResult<LuaTable> {
    let exports = lua.create_table()?;
    exports.set("load", lua.create_function(load)?)?;
    exports.set("run", lua.create_function(run)?)?;
    exports.set("join", lua.create_function(join)?)?;
    exports.set("call", lua.create_function(call)?)?;
    Ok(exports)
}
