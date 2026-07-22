use std::any::Any;
use std::thread::JoinHandle;

use anyhow::Result;
use mlua::prelude::*;
use mlua::Value;
use tarantool::coio::coio_call;

use crate::runtime::convert::{from_rmpv, to_rmpv};
use crate::runtime::tarantool_wasm::{
    debug_assert_tx_thread, tt_wasm_call_function, tt_wasm_call_functions, tt_wasm_close_session,
    tt_wasm_drop_module, tt_wasm_export_names, tt_wasm_interpreter_call, tt_wasm_interpreter_exec,
    tt_wasm_interpreter_key, tt_wasm_load_module, tt_wasm_open_session,
    tt_wasm_precompile_component, tt_wasm_run_module, tt_wasm_session_call_function,
    tt_wasm_session_call_functions, WasmRunHandle,
};

use super::args::{
    parse_batch_calls, parse_calls_table, parse_function_selector, parse_required_string,
};
use super::config::parse_config_table;

/// A lightweight wrapper around [`WasmRunHandle`] used to expose join
/// operations to the Lua side.
struct WasmJoinHandle(WasmRunHandle);

impl LuaUserData for WasmJoinHandle {}
impl FromLua for WasmJoinHandle {
    fn from_lua(value: Value, _lua: &'_ Lua) -> std::result::Result<WasmJoinHandle, mlua::Error> {
        value
            .as_userdata()
            .ok_or(LuaError::runtime("argument is not a join handle"))?
            .take()
    }
}

impl WasmJoinHandle {
    fn cancel(&self) -> Result<()> {
        self.0.cancel()
    }

    fn take_join_handle(&mut self) -> Result<JoinHandle<Result<()>>> {
        self.0.take_join_handle()
    }
}

fn panic_payload_to_string(payload: Box<dyn Any + Send + 'static>) -> String {
    if let Some(msg) = payload.downcast_ref::<&str>() {
        return (*msg).to_string();
    }
    if let Some(msg) = payload.downcast_ref::<String>() {
        return msg.clone();
    }
    "unknown panic payload".to_string()
}

fn join_result_to_anyhow(result: std::thread::Result<Result<()>>) -> Result<()> {
    match result {
        Ok(inner) => inner,
        Err(payload) => Err(anyhow::anyhow!(
            "worker thread panicked: {}",
            panic_payload_to_string(payload)
        )),
    }
}

/// Load a WebAssembly component and return its unique identifier.
///
/// The first argument is a path to the component binary. The optional second
/// argument is a table with configuration options understood by
/// [`crate::runtime::tarantool_wasm::WasmConfig`].
fn load(lua: &Lua, args: LuaMultiValue) -> LuaResult<String> {
    let mut iter = args.into_iter();
    let path = parse_required_string(iter.next(), "expected path as string")?;

    let cfg = match iter.next() {
        Some(value) => {
            let tbl: LuaTable = LuaTable::from_lua(value, lua)?;
            parse_config_table(Some(tbl))?
        }
        None => parse_config_table(None)?,
    };

    tt_wasm_load_module(path, cfg).map_err(LuaError::runtime)
}

/// Precompile a component for faster subsequent loads.
///
/// The optional third argument accepts the same configuration table as
/// [`load`]. Only engine-affecting options, currently `max_instructions`, are
/// used while producing the artifact.
fn precompile(lua: &Lua, args: LuaMultiValue) -> LuaResult<bool> {
    let mut iter = args.into_iter();
    let input = parse_required_string(iter.next(), "expected input path as string")?;
    let output = parse_required_string(iter.next(), "expected output path as string")?;
    let cfg = match iter.next() {
        Some(value) => {
            let tbl: LuaTable = LuaTable::from_lua(value, lua)?;
            parse_config_table(Some(tbl))?
        }
        None => parse_config_table(None)?,
    };

    tt_wasm_precompile_component(input, output, cfg).map_err(LuaError::runtime)?;
    Ok(true)
}

/// Start execution of a previously loaded module in a background fiber.
///
/// Returns a [`WasmJoinHandle`] that can be used to wait for completion.
fn run(lua: &Lua, args: LuaMultiValue) -> LuaResult<WasmJoinHandle> {
    let mut iter = args.into_iter();
    let module_uid = parse_required_string(iter.next(), "expected module_uid as string")?;

    let cfg = match iter.next() {
        Some(value) => {
            let tbl: LuaTable = LuaTable::from_lua(value, lua)?;
            Some(parse_config_table(Some(tbl))?)
        }
        None => None,
    };
    tt_wasm_run_module(module_uid, cfg)
        .map(WasmJoinHandle)
        .map_err(LuaError::runtime)
}

/// Open a long-lived session for repeated calls to the same module instance.
///
/// The first argument is the module identifier. The optional second argument is
/// an execution config table overriding the module defaults for this session.
fn open_session(lua: &Lua, args: LuaMultiValue) -> LuaResult<String> {
    let mut iter = args.into_iter();
    let module_uid = parse_required_string(iter.next(), "expected module_uid as string")?;

    let cfg = match iter.next() {
        Some(value) => {
            let tbl: LuaTable = LuaTable::from_lua(value, lua)?;
            Some(parse_config_table(Some(tbl))?)
        }
        None => None,
    };

    tt_wasm_open_session(module_uid, cfg).map_err(LuaError::runtime)
}

/// Wait for a background task represented by [`WasmJoinHandle`].
fn join(_: &Lua, handle: WasmJoinHandle) -> LuaResult<bool> {
    debug_assert_tx_thread();
    let mut handle = handle;
    let mut result: Result<()> = Ok(());
    let join_handle = handle.take_join_handle().map_err(LuaError::runtime)?;
    coio_call(
        &mut |handle: Box<JoinHandle<Result<()>>>| {
            result = join_result_to_anyhow(handle.join());
            0
        },
        join_handle,
    );
    match result {
        Ok(()) => Ok(true),
        Err(err) => Err(LuaError::runtime(err)),
    }
}

/// Cancel execution of a background task represented by [`WasmJoinHandle`].
fn cancel(_: &Lua, handle: WasmJoinHandle) -> LuaResult<bool> {
    debug_assert_tx_thread();
    let mut handle = handle;
    handle.cancel().map_err(LuaError::runtime)?;
    let mut result: Result<()> = Ok(());
    let join_handle = handle.take_join_handle().map_err(LuaError::runtime)?;
    coio_call(
        &mut |handle: Box<JoinHandle<Result<()>>>| {
            result = join_result_to_anyhow(handle.join());
            0
        },
        join_handle,
    );
    match result {
        Ok(()) => Ok(true),
        Err(err) => Err(LuaError::runtime(err)),
    }
}

/// Call a function exported by a loaded module.
///
/// The first argument is the module identifier, the second is the function
/// name (optionally prefixed with `"interface::"`). Remaining values are
/// passed as arguments.
fn call(lua: &Lua, args: LuaMultiValue) -> LuaResult<Value> {
    let mut iter = args.into_iter();

    let module_uid = parse_required_string(iter.next(), "expected module_uid as string")?;
    let (interface, func) = parse_function_selector(iter.next())?;
    let rest_args: Vec<Value> = iter.collect();

    let rmp_args = rest_args
        .iter()
        .map(|v| to_rmpv(lua, v))
        .collect::<Result<Vec<_>, _>>()?;

    let result_val = tt_wasm_call_function(module_uid, interface, func, rmp_args)
        .map_err(mlua::Error::external)?;

    let lua_val = from_rmpv(lua, &result_val)?;
    Ok(lua_val)
}

/// Execute multiple function calls in a single roundtrip to the WASM runtime.
fn batch(lua: &Lua, args: LuaMultiValue) -> LuaResult<Value> {
    let mut iter = args.into_iter();
    let module_uid = parse_required_string(iter.next(), "expected module_uid as string")?;
    let calls_tbl = parse_calls_table(iter.next())?;
    let calls = parse_batch_calls(lua, calls_tbl)?;

    let result_vals = tt_wasm_call_functions(module_uid, calls).map_err(mlua::Error::external)?;

    let results_tbl = lua.create_table()?;
    for (i, val) in result_vals.into_iter().enumerate() {
        let lua_val = from_rmpv(lua, &val)?;
        results_tbl.set(i + 1, lua_val)?;
    }

    Ok(LuaValue::Table(results_tbl))
}

/// Execute source code in a module loaded with `{ interpreter = true }`.
fn interpreter_exec(_: &Lua, args: LuaMultiValue) -> LuaResult<bool> {
    let mut iter = args.into_iter();
    let module_uid = parse_required_string(iter.next(), "expected module_uid as string")?;
    let code = parse_required_string(iter.next(), "expected code as string")?;

    tt_wasm_interpreter_exec(module_uid, code).map_err(mlua::Error::external)?;
    Ok(true)
}

/// Call a function in a module loaded with `{ interpreter = true }`.
fn interpreter_call(lua: &Lua, args: LuaMultiValue) -> LuaResult<Value> {
    let mut iter = args.into_iter();
    let module_uid = parse_required_string(iter.next(), "expected module_uid as string")?;
    let func = parse_required_string(iter.next(), "expected function name as string")?;
    let rest_args: Vec<Value> = iter.collect();
    let rmp_args = rest_args
        .iter()
        .map(|v| to_rmpv(lua, v))
        .collect::<Result<Vec<_>, _>>()?;

    let result_val =
        tt_wasm_interpreter_call(module_uid, func, rmp_args).map_err(mlua::Error::external)?;
    from_rmpv(lua, &result_val)
}

fn interpreter_key(_: &Lua, module_uid: String) -> LuaResult<String> {
    tt_wasm_interpreter_key(module_uid).map_err(mlua::Error::external)
}

fn create_interpreter_functions(lua: &Lua) -> LuaResult<(LuaFunction, LuaFunction)> {
    let raw_exec = lua.create_function(interpreter_exec)?;
    let raw_call = lua.create_function(interpreter_call)?;
    let key_for = lua.create_function(interpreter_key)?;

    lua.load(
        r#"
        local raw_exec, raw_call, key_for = ...
        local fiber = require('fiber')
        local locks = setmetatable({}, {__mode = 'v'})

        local function pack(...)
            return {n = select('#', ...), ...}
        end

        local function serialized(raw, ...)
            local module_uid = select(1, ...)
            local key = key_for(module_uid)
            local lock = locks[key]
            if lock == nil then
                lock = fiber.channel(1)
                lock:put(true)
                locks[key] = lock
            end

            lock:get()
            local result = pack(pcall(raw, ...))
            lock:put(true)

            if not result[1] then
                error(result[2], 0)
            end
            return unpack(result, 2, result.n)
        end

        return function(...)
            return serialized(raw_exec, ...)
        end, function(...)
            return serialized(raw_call, ...)
        end
        "#,
    )
    .call((raw_exec, raw_call, key_for))
}

/// Call a function on an already opened long-lived WASM session.
fn session_call(lua: &Lua, args: LuaMultiValue) -> LuaResult<Value> {
    let mut iter = args.into_iter();

    let session_handle = parse_required_string(iter.next(), "expected session_handle as string")?;
    let (interface, func) = parse_function_selector(iter.next())?;
    let rest_args: Vec<Value> = iter.collect();

    let rmp_args = rest_args
        .iter()
        .map(|v| to_rmpv(lua, v))
        .collect::<Result<Vec<_>, _>>()?;

    let result_val = tt_wasm_session_call_function(session_handle, interface, func, rmp_args)
        .map_err(mlua::Error::external)?;

    let lua_val = from_rmpv(lua, &result_val)?;
    Ok(lua_val)
}

/// Execute multiple function calls on an already opened long-lived WASM session.
fn session_batch(lua: &Lua, args: LuaMultiValue) -> LuaResult<Value> {
    let mut iter = args.into_iter();

    let session_handle = parse_required_string(iter.next(), "expected session_handle as string")?;
    let calls_tbl = parse_calls_table(iter.next())?;
    let calls = parse_batch_calls(lua, calls_tbl)?;

    let result_vals =
        tt_wasm_session_call_functions(session_handle, calls).map_err(mlua::Error::external)?;

    let results_tbl = lua.create_table()?;
    for (i, val) in result_vals.into_iter().enumerate() {
        let lua_val = from_rmpv(lua, &val)?;
        results_tbl.set(i + 1, lua_val)?;
    }

    Ok(LuaValue::Table(results_tbl))
}

/// Close a long-lived WASM session.
fn close_session(_: &Lua, session_handle: String) -> LuaResult<bool> {
    tt_wasm_close_session(session_handle).map_err(LuaError::runtime)?;
    Ok(true)
}

/// Return a Lua table describing all exported functions of a module.
fn exports(lua: &Lua, module_uid: String) -> LuaResult<Value> {
    match tt_wasm_export_names(module_uid) {
        Ok(export_map) => {
            let result = lua.create_table()?;

            for (namespace, funcs) in export_map {
                let func_table = lua.create_table()?;

                for (func_name, info) in funcs {
                    let entry = lua.create_table()?;

                    let param_list = lua.create_table()?;
                    for (i, (name, typ)) in info.params.iter().enumerate() {
                        let pair = lua.create_table()?;
                        pair.set(1, name.clone())?;
                        pair.set(2, typ.clone())?;
                        param_list.set(i + 1, pair)?;
                    }

                    let result_list = lua.create_table()?;
                    for (i, typ) in info.results.iter().enumerate() {
                        result_list.set(i + 1, typ.clone())?;
                    }

                    entry.set("params", param_list)?;
                    entry.set("result", result_list)?;
                    func_table.set(func_name, entry)?;
                }

                if namespace.is_empty() {
                    for pair in func_table.pairs::<Value, Value>() {
                        let (k, v) = pair?;
                        result.set(k, v)?;
                    }
                } else {
                    result.set(namespace, func_table)?;
                }
            }

            Ok(LuaValue::Table(result))
        }
        Err(e) => Err(mlua::Error::external(e)),
    }
}

/// Drop a previously loaded module and free associated resources.
fn drop(_: &Lua, module_uid: String) -> LuaResult<bool> {
    match tt_wasm_drop_module(module_uid) {
        Ok(()) => Ok(true),
        Err(e) => Err(LuaError::runtime(e)),
    }
}

pub(crate) fn create_module(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    let (serialized_interpreter_exec, serialized_interpreter_call) =
        create_interpreter_functions(lua)?;
    module.set("load", lua.create_function(load)?)?;
    module.set("precompile", lua.create_function(precompile)?)?;
    module.set("run", lua.create_function(run)?)?;
    module.set("open_session", lua.create_function(open_session)?)?;
    module.set("join", lua.create_function(join)?)?;
    module.set("cancel", lua.create_function(cancel)?)?;
    module.set("call", lua.create_function(call)?)?;
    module.set("batch", lua.create_function(batch)?)?;
    module.set("interpreter_exec", serialized_interpreter_exec)?;
    module.set("interpreter_call", serialized_interpreter_call)?;
    module.set("session_call", lua.create_function(session_call)?)?;
    module.set("session_batch", lua.create_function(session_batch)?)?;
    module.set("close_session", lua.create_function(close_session)?)?;
    module.set("exports", lua.create_function(exports)?)?;
    module.set("drop", lua.create_function(drop)?)?;
    Ok(module)
}
