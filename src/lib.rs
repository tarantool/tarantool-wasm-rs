//! Lua bindings for executing WebAssembly modules inside Tarantool.
//!
//! The functions exposed from this crate are intended to be loaded from Lua
//! and provide high level access to the WASI-enabled Wasmtime runtime. Each
//! helper translates Lua values into the structures expected by the runtime and
//! back, hiding the underlying complexity from the user.

mod bindings;
mod ffi;
mod runtime;

use crate::runtime::convert::{from_rmpv, to_rmpv};
use crate::runtime::tarantool_wasm::{
    tt_wasm_call_function, tt_wasm_call_functions, tt_wasm_drop_module, tt_wasm_export_names,
    tt_wasm_load_module, tt_wasm_run_module, WasmConfig, WasmRunHandle,
};
use anyhow::Result;
use mlua::prelude::*;
use mlua::Value;
use std::any::Any;
use std::thread::JoinHandle;
use tarantool::coio::coio_call;
use wasmtime_wasi::{DirPerms, FilePerms};

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

/// Parse a Lua table into a [`WasmConfig`].
///
/// Every configuration option is optional; if a field is absent the default
/// value from [`WasmConfig::default`] is used.
fn parse_config_table(tbl: Option<LuaTable>) -> LuaResult<WasmConfig> {
    let mut cfg = WasmConfig::default();
    if let Some(t) = tbl {
        if let Ok(Some(inherit_env)) = t.get::<Option<bool>>("inherit_env") {
            cfg.inherit_env = inherit_env;
        }
        if let Ok(Some(inherit_args)) = t.get::<Option<bool>>("inherit_args") {
            cfg.inherit_args = inherit_args;
        }
        if let Ok(args_tbl) = t.get::<LuaTable>("args") {
            for val in args_tbl.sequence_values::<String>() {
                cfg.args.push(val?);
            }
        }
        if let Ok(Some(inherit_stdin)) = t.get::<Option<bool>>("inherit_stdin") {
            cfg.inherit_stdin = inherit_stdin;
        }
        if let Ok(Some(inherit_stdout)) = t.get::<Option<bool>>("inherit_stdout") {
            cfg.inherit_stdout = inherit_stdout;
        }
        if let Ok(Some(inherit_stderr)) = t.get::<Option<bool>>("inherit_stderr") {
            cfg.inherit_stderr = inherit_stderr;
        }
        if let Ok(stdin) = t.get::<String>("stdin") {
            cfg.stdin = Some(stdin);
        }
        if let Ok(stdout) = t.get::<String>("stdout") {
            cfg.stdout = Some(stdout);
        }
        if let Ok(stderr) = t.get::<String>("stderr") {
            cfg.stderr = Some(stderr);
        }
        if let Ok(env_tbl) = t.get::<LuaTable>("env") {
            for pair in env_tbl.pairs::<String, String>() {
                let (k, v) = pair?;
                cfg.env.insert(k, v);
            }
        }
        if let Ok(Some(inherit_network)) = t.get::<Option<bool>>("inherit_network") {
            cfg.inherit_network = inherit_network;
        }
        if let Ok(Some(allow_ip_name_lookup)) = t.get::<Option<bool>>("allow_ip_name_lookup") {
            cfg.allow_ip_name_lookup = allow_ip_name_lookup;
        }
        if let Ok(Some(allow_tcp)) = t.get::<Option<bool>>("allow_tcp") {
            cfg.allow_tcp = allow_tcp;
        }
        if let Ok(Some(allow_udp)) = t.get::<Option<bool>>("allow_udp") {
            cfg.allow_udp = allow_udp;
        }
        if let Ok(ip_tbl) = t.get::<LuaTable>("allowed_ips") {
            for ip in ip_tbl.sequence_values::<String>() {
                cfg.allowed_ips.push(ip?);
            }
        }
        if let Ok(port_tbl) = t.get::<LuaTable>("allowed_ports") {
            for port in port_tbl.sequence_values::<u16>() {
                cfg.allowed_ports.push(port?);
            }
        }
        if let Ok(po_tbl) = t.get::<LuaTable>("preopened_dirs") {
            for entry in po_tbl.sequence_values::<LuaTable>() {
                let tbl = entry?;
                let host: String = tbl.raw_get(1)?;
                let guest: String = tbl.raw_get(2)?;

                let perms: Option<String> = tbl.raw_get("perms").ok();
                let (dir_perms, file_perms) = match perms.as_deref() {
                    Some("ro") => (DirPerms::READ, FilePerms::READ),
                    _ => (DirPerms::all(), FilePerms::all()),
                };

                cfg.preopened_dirs
                    .push((host, guest, dir_perms, file_perms));
            }
        }
        if let Ok(memory_limit) = t.get::<u64>("memory_limit") {
            cfg.memory_limit = Some(memory_limit as usize);
        }
        if let Ok(max_instructions) = t.get::<u64>("max_instructions") {
            cfg.max_instructions = Some(max_instructions);
        }
        if let Ok(pool_size) = t.get::<u64>("pool_size") {
            cfg.pool_size = pool_size as usize;
        }
        if let Ok(warm_instances) = t.get::<u64>("warm_instances") {
            cfg.warm_instances = warm_instances as usize;
        }
    }
    Ok(cfg)
}

/// Load a WebAssembly component and return its unique identifier.
///
/// The first argument is a path to the component binary. The optional second
/// argument is a table with configuration options understood by
/// [`WasmConfig`].
fn load(lua: &Lua, args: LuaMultiValue) -> LuaResult<String> {
    let mut iter = args.into_iter();
    let path = match iter.next() {
        Some(Value::String(s)) => s.to_str()?.to_string(),
        _ => {
            return Err(mlua::Error::FromLuaConversionError {
                from: "Value",
                to: "String".to_string(),
                message: Some("expected path as string".to_string()),
            });
        }
    };

    let cfg = match iter.next() {
        Some(v) => {
            let tbl: LuaTable = LuaTable::from_lua(v, lua)?;
            parse_config_table(Some(tbl))?
        }
        None => WasmConfig::default(),
    };

    tt_wasm_load_module(path, cfg).map_err(LuaError::runtime)
}

/// Start execution of a previously loaded module in a background fiber.
///
/// Returns a [`WasmJoinHandle`] that can be used to wait for completion.
fn run(lua: &Lua, args: LuaMultiValue) -> LuaResult<WasmJoinHandle> {
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

    let cfg = match iter.next() {
        Some(v) => {
            let tbl: LuaTable = LuaTable::from_lua(v, lua)?;
            Some(parse_config_table(Some(tbl))?)
        }
        None => None,
    };
    tt_wasm_run_module(module_uid, cfg)
        .map(WasmJoinHandle)
        .map_err(LuaError::runtime)
}

/// Wait for a background task represented by [`WasmJoinHandle`].
fn join(_: &Lua, handle: WasmJoinHandle) -> LuaResult<bool> {
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

    let (interface, func) = match iter.next() {
        Some(Value::String(s)) => {
            let s = s.to_str()?;
            match s.split_once("::") {
                Some((iface, func)) => (Some(iface.to_string()), func.to_string()),
                None => (None, s.to_string()),
            }
        }
        _ => {
            return Err(mlua::Error::FromLuaConversionError {
                from: "Value",
                to: "String".to_string(),
                message: Some("expected function name as string".to_string()),
            });
        }
    };

    let rest_args: Vec<Value> = iter.collect();

    let rmp_args = rest_args
        .iter()
        .map(to_rmpv)
        .collect::<Result<Vec<_>, _>>()?;

    let result_val = tt_wasm_call_function(module_uid, interface, func, rmp_args)
        .map_err(mlua::Error::external)?;

    let lua_val = from_rmpv(lua, &result_val)?;
    Ok(lua_val)
}

/// Execute multiple function calls in a single roundtrip to the WASM runtime.
fn batch(lua: &Lua, args: LuaMultiValue) -> LuaResult<Value> {
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

        let (interface, func): (Option<String>, String) = match call_tbl.raw_get(1)? {
            LuaValue::String(s) => {
                let s = s.to_str()?;
                match s.split_once("::") {
                    Some((iface, func)) => (Some(iface.to_string()), func.to_string()),
                    None => (None, s.to_string()),
                }
            }
            _ => {
                return Err(mlua::Error::FromLuaConversionError {
                    from: "Value",
                    to: "string".to_string(),
                    message: Some(
                        "call entry first element must be a function name string".to_string(),
                    ),
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

        calls.push((interface, func, rmp_args));
    }

    let result_vals = tt_wasm_call_functions(module_uid, calls).map_err(mlua::Error::external)?;

    let results_tbl = lua.create_table()?;
    for (i, val) in result_vals.into_iter().enumerate() {
        let lua_val = from_rmpv(lua, &val)?;
        results_tbl.set(i + 1, lua_val)?;
    }

    Ok(LuaValue::Table(results_tbl))
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

/// Register the `wasm` Lua module with Tarantool.
#[mlua::lua_module(name = "wasm")]
fn my_module(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set("load", lua.create_function(load)?)?;
    module.set("run", lua.create_function(run)?)?;
    module.set("join", lua.create_function(join)?)?;
    module.set("cancel", lua.create_function(cancel)?)?;
    module.set("call", lua.create_function(call)?)?;
    module.set("batch", lua.create_function(batch)?)?;
    module.set("exports", lua.create_function(exports)?)?;
    module.set("drop", lua.create_function(drop)?)?;
    Ok(module)
}
