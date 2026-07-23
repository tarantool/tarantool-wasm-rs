use std::any::Any;
use std::sync::Arc;
use std::thread::JoinHandle;

use anyhow::Result;
use mlua::prelude::*;
use mlua::Value;
use tarantool::coio::coio_call;

use super::resource::{
    from_rmpv_with_instance, mark_consumed, to_rmpv_with_instance_tracking, InstanceLease,
    LuaWasmResource,
};
use crate::bindings::lua_callback::callback_registry_key;
use crate::runtime::convert::{from_rmpv, to_rmpv};
use crate::runtime::tarantool_wasm::{
    debug_assert_tx_thread, tt_wasm_bind_instance, tt_wasm_call_function, tt_wasm_call_functions,
    tt_wasm_close_session, tt_wasm_drop_module, tt_wasm_export_names,
    tt_wasm_instance_call_function_with_ownership, tt_wasm_interpreter_call,
    tt_wasm_interpreter_exec, tt_wasm_interpreter_key, tt_wasm_load_module, tt_wasm_open_session,
    tt_wasm_precompile_component, tt_wasm_run_module, tt_wasm_session_call_function_with_ownership,
    tt_wasm_session_call_functions_with_ownership, WasmInstanceTarget, WasmRunHandle,
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

    let instance = InstanceLease::borrowed(WasmInstanceTarget::Session(session_handle.clone()));
    let mut used = Vec::new();
    let rmp_args = rest_args
        .iter()
        .map(|value| to_rmpv_with_instance_tracking(lua, value, instance.as_ref(), &mut used))
        .collect::<Result<Vec<_>, _>>()?;

    let result =
        tt_wasm_session_call_function_with_ownership(session_handle, interface, func, rmp_args)
            .map_err(mlua::Error::external)?;
    mark_consumed(&used, &result.consumed);

    let lua_val = from_rmpv_with_instance(lua, &result.value, instance)?;
    Ok(lua_val)
}

/// Execute multiple function calls on an already opened long-lived WASM session.
fn session_batch(lua: &Lua, args: LuaMultiValue) -> LuaResult<Value> {
    let mut iter = args.into_iter();

    let session_handle = parse_required_string(iter.next(), "expected session_handle as string")?;
    let calls_tbl = parse_calls_table(iter.next())?;
    let instance = InstanceLease::borrowed(WasmInstanceTarget::Session(session_handle.clone()));
    let mut calls = Vec::new();
    let mut used: Vec<LuaWasmResource> = Vec::new();
    for call in calls_tbl.sequence_values::<LuaValue>() {
        let call = LuaTable::from_lua(call?, lua)?;
        let (interface, func) = match call.raw_get(1)? {
            LuaValue::String(selector) => {
                super::args::split_function_selector(selector.to_str()?.as_ref())
            }
            _ => return Err(LuaError::runtime("call function name must be a string")),
        };
        let mut call_args = Vec::new();
        for index in 2..=call.raw_len() {
            let value = call.raw_get(index)?;
            call_args.push(to_rmpv_with_instance_tracking(
                lua,
                &value,
                instance.as_ref(),
                &mut used,
            )?);
        }
        calls.push((interface, func, call_args));
    }

    let results = tt_wasm_session_call_functions_with_ownership(session_handle, calls)
        .map_err(mlua::Error::external)?;

    let results_tbl = lua.create_table()?;
    for (i, result) in results.into_iter().enumerate() {
        mark_consumed(&used, &result.consumed);
        let lua_val = from_rmpv_with_instance(lua, &result.value, Arc::clone(&instance))?;
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

fn create_bound_export(
    lua: &Lua,
    instance: Arc<InstanceLease>,
    interface: Option<String>,
    func: String,
) -> LuaResult<LuaFunction> {
    lua.create_function(move |lua, args: LuaMultiValue| {
        let mut used = Vec::new();
        let rmp_args = args
            .iter()
            .map(|value| to_rmpv_with_instance_tracking(lua, value, instance.as_ref(), &mut used))
            .collect::<Result<Vec<_>, _>>()?;
        let result = tt_wasm_instance_call_function_with_ownership(
            instance.target(),
            interface.clone(),
            func.clone(),
            rmp_args,
        )
        .map_err(mlua::Error::external)?;
        mark_consumed(&used, &result.consumed);
        from_rmpv_with_instance(lua, &result.value, Arc::clone(&instance))
    })
}

fn resource_member(func: &str) -> Option<(&str, &str)> {
    if let Some(resource) = func.strip_prefix("[constructor]") {
        return Some((resource, "new"));
    }
    let rest = func.strip_prefix("[static]")?;
    rest.split_once('.')
}

/// Bind the exports of a loaded module to Lua functions.
fn bind(
    lua: &Lua,
    module_uid: String,
    serialized_interpreter_exec: &LuaFunction,
    serialized_interpreter_call: &LuaFunction,
) -> LuaResult<LuaTable> {
    let export_map = tt_wasm_export_names(module_uid.clone()).map_err(mlua::Error::external)?;
    let instance = InstanceLease::owned(
        tt_wasm_bind_instance(module_uid.clone()).map_err(mlua::Error::external)?,
    );
    let is_interpreter = matches!(instance.target(), WasmInstanceTarget::Interpreter(_));
    let proxy = lua.create_table()?;
    for (namespace, funcs) in export_map {
        let interface = (!namespace.is_empty()).then_some(namespace.clone());
        let target = if interface.is_some() {
            lua.create_table()?
        } else {
            proxy.clone()
        };
        for func_name in funcs.into_keys() {
            let bound = create_bound_export(
                lua,
                Arc::clone(&instance),
                interface.clone(),
                func_name.clone(),
            )?;
            target.set(func_name.clone(), bound.clone())?;
            if let Some((resource, member)) = resource_member(&func_name) {
                let resource_table = match target.get(resource)? {
                    LuaValue::Table(table) => table,
                    LuaValue::Nil => {
                        let table = lua.create_table()?;
                        target.set(resource, table.clone())?;
                        table
                    }
                    _ => {
                        return Err(LuaError::runtime(format!(
                            "resource binding `{resource}` conflicts with an export"
                        )))
                    }
                };
                resource_table.set(member, bound)?;
            }
        }
        if interface.is_some() {
            proxy.set(namespace, target)?;
        }
    }
    if is_interpreter {
        let (bound_exec, bound_call): (LuaFunction, LuaFunction) = lua
            .load(
                r#"
                local interpreter_exec, interpreter_call, module_uid = ...
                return function(code)
                    return interpreter_exec(module_uid, code)
                end, function(func, ...)
                    return interpreter_call(module_uid, func, ...)
                end
                "#,
            )
            .call((
                serialized_interpreter_exec.clone(),
                serialized_interpreter_call.clone(),
                module_uid,
            ))?;
        proxy.set("interpreter_exec", bound_exec)?;
        proxy.set("interpreter_call", bound_call)?;
    }
    Ok(proxy)
}

fn create_bind_function(
    lua: &Lua,
    serialized_interpreter_exec: LuaFunction,
    serialized_interpreter_call: LuaFunction,
) -> LuaResult<LuaFunction> {
    lua.create_function(move |lua, module_uid: String| {
        bind(
            lua,
            module_uid,
            &serialized_interpreter_exec,
            &serialized_interpreter_call,
        )
    })
}

/// Drop a previously loaded module and free associated resources.
fn drop(_: &Lua, module_uid: String) -> LuaResult<bool> {
    match tt_wasm_drop_module(module_uid) {
        Ok(()) => Ok(true),
        Err(e) => Err(LuaError::runtime(e)),
    }
}

/// Keep a Lua closure alive under a stable name that guest components can call.
fn register_callback(
    lua: &Lua,
    (name, callback, options): (String, LuaFunction, Option<LuaTable>),
) -> LuaResult<bool> {
    if name.is_empty() {
        return Err(LuaError::runtime("callback name must not be empty"));
    }

    let timeout_ms = options
        .map(|options| options.get::<Option<u64>>("timeout_ms"))
        .transpose()?
        .flatten();
    if timeout_ms == Some(0) {
        return Err(LuaError::runtime(
            "callback timeout_ms must be greater than zero",
        ));
    }

    let registration = lua.create_table()?;
    registration.set("callback", callback)?;
    if let Some(timeout_ms) = timeout_ms {
        registration.set("timeout_ms", timeout_ms)?;
    }
    lua.set_named_registry_value(&callback_registry_key(&name), registration)?;
    Ok(true)
}

/// Remove a previously registered Lua callback.
fn unregister_callback(lua: &Lua, name: String) -> LuaResult<bool> {
    if name.is_empty() {
        return Err(LuaError::runtime("callback name must not be empty"));
    }
    let key = callback_registry_key(&name);
    let existed = !matches!(lua.named_registry_value::<LuaValue>(&key)?, LuaValue::Nil);
    lua.unset_named_registry_value(&key)?;
    Ok(existed)
}

pub(crate) fn create_module(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    let (serialized_interpreter_exec, serialized_interpreter_call) =
        create_interpreter_functions(lua)?;
    let bind = create_bind_function(
        lua,
        serialized_interpreter_exec.clone(),
        serialized_interpreter_call.clone(),
    )?;
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
    module.set("bind", bind)?;
    module.set("drop", lua.create_function(drop)?)?;
    module.set("register_callback", lua.create_function(register_callback)?)?;
    module.set(
        "unregister_callback",
        lua.create_function(unregister_callback)?,
    )?;
    Ok(module)
}
