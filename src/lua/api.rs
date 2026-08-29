use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use mlua::prelude::*;
use mlua::Value;
use rmpv::Value as RmpvValue;
use tarantool::coio::coio_call;

use super::resource::{
    from_rmpv_with_instance, mark_consumed, to_rmpv_with_instance_tracking, InstanceLease,
    LuaWasmResource,
};
use crate::bindings::lua_callback::callback_registry_key;
use crate::runtime::convert::{from_rmpv, msgpack_bytes_to_rmpv, rmpv_to_msgpack_bytes, to_rmpv};
use crate::runtime::metrics::{self, TimingSnapshot};
use crate::runtime::tarantool_wasm::{
    debug_assert_tx_thread, start_wasm_call_function, start_wasm_call_functions,
    tt_wasm_bind_instance, tt_wasm_close_session, tt_wasm_drop_module, tt_wasm_export_names,
    tt_wasm_instance_call_function_with_ownership, tt_wasm_interpreter_call,
    tt_wasm_interpreter_exec, tt_wasm_interpreter_key, tt_wasm_load_module, tt_wasm_open_session,
    tt_wasm_pipe_spawn, tt_wasm_precompile_component, tt_wasm_run_module,
    tt_wasm_session_call_function_with_ownership, tt_wasm_session_call_functions_with_ownership,
    tt_wasm_stats, PipeStream, WasmCallHandle, WasmInstanceTarget, WasmPipeProcess, WasmRunHandle,
    DEFAULT_PIPE_CAPACITY, MAX_PIPE_CAPACITY,
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
    fn from_lua(value: Value, _lua: &'_ Lua) -> std::result::Result<Self, mlua::Error> {
        value
            .as_userdata()
            .ok_or_else(|| LuaError::runtime("argument is not a join handle"))?
            .take()
    }
}

struct LuaCallState {
    handle: Option<WasmCallHandle<RmpvValue>>,
    result: Option<Result<RmpvValue>>,
}

struct LuaCallHandle(Mutex<LuaCallState>);

impl LuaUserData for LuaCallHandle {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("wait", |_, this, ()| {
            let handle = this
                .0
                .lock()
                .map_err(|_| LuaError::runtime("call state lock poisoned"))?
                .handle
                .take()
                .ok_or_else(|| LuaError::runtime("call result already awaited"))?;
            let result = wait_for_call(handle);
            this.0
                .lock()
                .map_err(|_| LuaError::runtime("call state lock poisoned"))?
                .result = Some(result);
            Ok(true)
        });
        methods.add_method("take", |lua, this, ()| {
            let result = this
                .0
                .lock()
                .map_err(|_| LuaError::runtime("call state lock poisoned"))?
                .result
                .take()
                .ok_or_else(|| LuaError::runtime("call result is not ready"))?
                .map_err(LuaError::runtime)?;
            from_rmpv(lua, &result)
        });
    }
}

struct LuaBatchState {
    handle: Option<WasmCallHandle<Vec<RmpvValue>>>,
    result: Option<Result<Vec<RmpvValue>>>,
}

struct LuaBatchHandle(Mutex<LuaBatchState>);

impl LuaUserData for LuaBatchHandle {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("wait", |_, this, ()| {
            let handle = this
                .0
                .lock()
                .map_err(|_| LuaError::runtime("batch state lock poisoned"))?
                .handle
                .take()
                .ok_or_else(|| LuaError::runtime("batch result already awaited"))?;
            let result = wait_for_call(handle);
            this.0
                .lock()
                .map_err(|_| LuaError::runtime("batch state lock poisoned"))?
                .result = Some(result);
            Ok(true)
        });
        methods.add_method("take", |lua, this, ()| {
            let values = this
                .0
                .lock()
                .map_err(|_| LuaError::runtime("batch state lock poisoned"))?
                .result
                .take()
                .ok_or_else(|| LuaError::runtime("batch result is not ready"))?
                .map_err(LuaError::runtime)?;
            let results = lua.create_table()?;
            for (index, value) in values.into_iter().enumerate() {
                results.set(index + 1, from_rmpv(lua, &value)?)?;
            }
            Ok(LuaValue::Table(results))
        });
    }
}

fn wait_for_call<T: Send + 'static>(handle: WasmCallHandle<T>) -> Result<T> {
    debug_assert_tx_thread();
    let started = std::time::Instant::now();
    let result = if handle.is_finished() {
        handle.join()
    } else {
        let mut result = Err(anyhow::anyhow!("failed to join WASM call worker"));
        coio_call(
            &mut |handle: Box<WasmCallHandle<T>>| {
                result = handle.join();
                0
            },
            handle,
        );
        result
    };
    metrics::observe_worker_wait(started.elapsed());
    result
}

struct LuaPipeProcess(WasmPipeProcess);

impl Drop for LuaPipeProcess {
    fn drop(&mut self) {
        self.0.abandon();
    }
}

fn pipe_timeout(options: Option<&LuaTable>) -> LuaResult<Option<Duration>> {
    let timeout_ms = options
        .map(|options| options.get::<Option<u64>>("timeout_ms"))
        .transpose()?
        .flatten();
    if timeout_ms == Some(0) {
        return Err(LuaError::runtime("timeout_ms must be greater than zero"));
    }
    Ok(timeout_ms.map(Duration::from_millis))
}

fn pipe_max_bytes(options: Option<&LuaTable>) -> LuaResult<usize> {
    let max_bytes = options
        .map(|options| options.get::<Option<usize>>("max_bytes"))
        .transpose()?
        .flatten()
        .unwrap_or(DEFAULT_PIPE_CAPACITY);
    if !(1..=MAX_PIPE_CAPACITY).contains(&max_bytes) {
        return Err(LuaError::runtime(format!(
            "max_bytes must be between 1 and {MAX_PIPE_CAPACITY}"
        )));
    }
    Ok(max_bytes)
}

fn pipe_coio<T, F>(operation: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    debug_assert_tx_thread();
    let mut result = Err(anyhow::anyhow!("pipe operation did not complete"));
    tarantool::coio::coio_call(
        &mut |operation: Box<F>| {
            result = operation();
            0
        },
        operation,
    );
    result
}

impl LuaUserData for LuaPipeProcess {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method(
            "write",
            |_, this, (data, options): (LuaString, Option<LuaTable>)| {
                let timeout = pipe_timeout(options.as_ref())?;
                let process = this.0.clone();
                let data = data.as_bytes().to_vec();
                pipe_coio(move || process.write(data, timeout)).map_err(LuaError::runtime)?;
                Ok(true)
            },
        );
        methods.add_method("read", |lua, this, options: Option<LuaTable>| {
            let stream = match options
                .as_ref()
                .map(|options| options.get::<Option<String>>("stream"))
                .transpose()?
                .flatten()
                .as_deref()
            {
                None | Some("stdout") => PipeStream::Stdout,
                Some("stderr") => PipeStream::Stderr,
                Some(value) => {
                    return Err(LuaError::runtime(format!("invalid pipe stream `{value}`")))
                }
            };
            let max_bytes = pipe_max_bytes(options.as_ref())?;
            let timeout = pipe_timeout(options.as_ref())?;
            let process = this.0.clone();
            let (data, eof) = pipe_coio(move || process.read(stream, max_bytes, timeout))
                .map_err(LuaError::runtime)?;
            Ok((lua.create_string(&data)?, eof))
        });
        methods.add_method("read_any", |lua, this, options: Option<LuaTable>| {
            let max_bytes = pipe_max_bytes(options.as_ref())?;
            let timeout = pipe_timeout(options.as_ref())?;
            let process = this.0.clone();
            let (stream, data, eof) = pipe_coio(move || process.read_any(max_bytes, timeout))
                .map_err(LuaError::runtime)?;
            let stream = stream.map(|stream| match stream {
                PipeStream::Stdout => "stdout",
                PipeStream::Stderr => "stderr",
            });
            Ok((stream, lua.create_string(&data)?, eof))
        });
        methods.add_method("close_stdin", |_, this, ()| {
            this.0.close_stdin();
            Ok(true)
        });
        methods.add_method("cancel", |_, this, ()| {
            this.0.cancel();
            Ok(true)
        });
        methods.add_method("join", |_, this, options: Option<LuaTable>| {
            let timeout = pipe_timeout(options.as_ref())?;
            let process = this.0.clone();
            pipe_coio(move || process.join(timeout)).map_err(LuaError::runtime)?;
            Ok(true)
        });
        methods.add_method("close", |_, this, options: Option<LuaTable>| {
            let timeout = pipe_timeout(options.as_ref())?;
            let process = this.0.clone();
            pipe_coio(move || process.close(timeout)).map_err(LuaError::runtime)?;
            Ok(true)
        });
        methods.add_method("is_alive", |_, this, ()| Ok(this.0.is_alive()));
    }
}

fn timing_stats_table(lua: &Lua, timing: TimingSnapshot) -> LuaResult<LuaTable> {
    let table = lua.create_table()?;
    table.set("count", timing.count)?;
    table.set("total_ns", timing.total_ns)?;
    table.set("max_ns", timing.max_ns)?;
    Ok(table)
}

fn stats(lua: &Lua, _: ()) -> LuaResult<LuaTable> {
    let snapshot = tt_wasm_stats();
    let metrics = snapshot.metrics;
    let result = lua.create_table()?;

    result.set("compilation", timing_stats_table(lua, metrics.compilation)?)?;
    result.set(
        "worker_start",
        timing_stats_table(lua, metrics.worker_start)?,
    )?;
    result.set(
        "runtime_setup",
        timing_stats_table(lua, metrics.runtime_setup)?,
    )?;
    result.set("store_setup", timing_stats_table(lua, metrics.store_setup)?)?;
    result.set(
        "instantiation",
        timing_stats_table(lua, metrics.instantiation)?,
    )?;
    result.set(
        "argument_lowering",
        timing_stats_table(lua, metrics.argument_lowering)?,
    )?;
    result.set(
        "guest_execution",
        timing_stats_table(lua, metrics.guest_execution)?,
    )?;
    result.set(
        "result_lifting",
        timing_stats_table(lua, metrics.result_lifting)?,
    )?;
    result.set("worker_wait", timing_stats_table(lua, metrics.worker_wait)?)?;
    result.set("tx_wait", timing_stats_table(lua, metrics.tx_wait)?)?;
    result.set(
        "lua_callback",
        timing_stats_table(lua, metrics.lua_callback)?,
    )?;

    let fuel = lua.create_table()?;
    fuel.set("executions", metrics.fuel.executions)?;
    fuel.set("consumed", metrics.fuel.consumed)?;
    result.set("fuel", fuel)?;

    let current = lua.create_table()?;
    current.set("modules", snapshot.modules)?;
    current.set("sessions", snapshot.sessions)?;
    current.set("runs", snapshot.runs)?;
    current.set("interpreters", snapshot.interpreters)?;
    result.set("current", current)?;

    let cache = lua.create_table()?;
    let components = lua.create_table()?;
    components.set("hits", metrics.component_cache.hits)?;
    components.set("misses", metrics.component_cache.misses)?;
    cache.set("components", components)?;
    let exports = lua.create_table()?;
    exports.set("hits", metrics.export_cache.hits)?;
    exports.set("misses", metrics.export_cache.misses)?;
    cache.set("exports", exports)?;
    result.set("cache", cache)?;

    let traps = lua.create_table()?;
    let trap_kinds = lua.create_table()?;
    let mut trap_total = 0_u64;
    for (kind, count) in metrics.traps {
        trap_total = trap_total.saturating_add(count);
        trap_kinds.set(kind, count)?;
    }
    traps.set("total", trap_total)?;
    traps.set("by_kind", trap_kinds)?;
    result.set("traps", traps)?;

    let conversion_errors = lua.create_table()?;
    let conversion_kinds = lua.create_table()?;
    let mut conversion_total = 0_u64;
    for (kind, count) in metrics.conversion_errors {
        conversion_total = conversion_total.saturating_add(count);
        conversion_kinds.set(kind, count)?;
    }
    conversion_errors.set("total", conversion_total)?;
    conversion_errors.set("by_kind", conversion_kinds)?;
    result.set("conversion_errors", conversion_errors)?;

    let memory = lua.create_table()?;
    memory.set("current_bytes", metrics.current_memory_bytes)?;
    memory.set("peak_bytes", metrics.peak_memory_bytes)?;
    result.set("memory", memory)?;

    Ok(result)
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

fn finish_run(_: &Lua, mut handle: WasmJoinHandle) -> LuaResult<bool> {
    handle.0.join().map_err(LuaError::runtime)?;
    Ok(true)
}

fn run_is_finished(_: &Lua, handle: LuaUserDataRef<WasmJoinHandle>) -> LuaResult<bool> {
    handle.0.ready_or_subscribe().map_err(LuaError::runtime)
}

fn request_run_cancel(_: &Lua, handle: LuaUserDataRef<WasmJoinHandle>) -> LuaResult<bool> {
    handle.0.cancel().map_err(LuaError::runtime)?;
    Ok(true)
}

fn pipe_spawn(lua: &Lua, args: LuaMultiValue) -> LuaResult<LuaPipeProcess> {
    let mut iter = args.into_iter();
    let module_uid = parse_required_string(iter.next(), "expected module_uid as string")?;
    let cfg = match iter.next() {
        Some(value) if !matches!(value, LuaValue::Nil) => {
            let table = LuaTable::from_lua(value, lua)?;
            for key in [
                "stdin",
                "stdout",
                "stderr",
                "inherit_stdin",
                "inherit_stdout",
                "inherit_stderr",
            ] {
                if !matches!(table.get::<LuaValue>(key)?, LuaValue::Nil) {
                    return Err(LuaError::runtime(format!(
                        "pipe spawn config must not set `{key}`"
                    )));
                }
            }
            Some(parse_config_table(Some(table))?)
        }
        _ => None,
    };
    let options = match iter.next() {
        Some(value) if !matches!(value, LuaValue::Nil) => Some(LuaTable::from_lua(value, lua)?),
        _ => None,
    };
    if iter.next().is_some() {
        return Err(LuaError::runtime("too many arguments to wasm.pipe.spawn"));
    }
    let capacity = options
        .as_ref()
        .map(|options| options.get::<Option<usize>>("capacity"))
        .transpose()?
        .flatten()
        .unwrap_or(DEFAULT_PIPE_CAPACITY);
    if !(1..=MAX_PIPE_CAPACITY).contains(&capacity) {
        return Err(LuaError::runtime(format!(
            "pipe capacity must be between 1 and {MAX_PIPE_CAPACITY} bytes"
        )));
    }
    tt_wasm_pipe_spawn(module_uid, cfg, capacity)
        .map(LuaPipeProcess)
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

/// Call a function exported by a loaded module.
///
/// The first argument is the module identifier, the second is the function
/// name (optionally prefixed with `"interface::"`). Remaining values are
/// passed as arguments.
fn call_start(lua: &Lua, args: LuaMultiValue) -> LuaResult<LuaCallHandle> {
    let mut iter = args.into_iter();

    let module_uid = parse_required_string(iter.next(), "expected module_uid as string")?;
    let (interface, func) = parse_function_selector(iter.next())?;
    let rest_args: Vec<Value> = iter.collect();

    let rmp_args = rest_args
        .iter()
        .map(|v| to_rmpv(lua, v))
        .collect::<Result<Vec<_>, _>>()?;

    start_wasm_call_function(module_uid, interface, func, rmp_args)
        .map(|handle| {
            LuaCallHandle(Mutex::new(LuaCallState {
                handle: Some(handle),
                result: None,
            }))
        })
        .map_err(mlua::Error::external)
}

/// Execute multiple function calls in a single roundtrip to the WASM runtime.
fn batch_start(lua: &Lua, args: LuaMultiValue) -> LuaResult<LuaBatchHandle> {
    let mut iter = args.into_iter();
    let module_uid = parse_required_string(iter.next(), "expected module_uid as string")?;
    let calls_tbl = parse_calls_table(iter.next())?;
    let calls = parse_batch_calls(lua, calls_tbl)?;

    start_wasm_call_functions(module_uid, calls)
        .map(|handle| {
            LuaBatchHandle(Mutex::new(LuaBatchState {
                handle: Some(handle),
                result: None,
            }))
        })
        .map_err(mlua::Error::external)
}

fn create_async_execution_functions(
    lua: &Lua,
) -> LuaResult<(LuaFunction, LuaFunction, LuaFunction, LuaFunction)> {
    let call_start = lua.create_function(call_start)?;
    let batch_start = lua.create_function(batch_start)?;
    let finish_run = lua.create_function(finish_run)?;
    let run_is_finished = lua.create_function(run_is_finished)?;
    let request_run_cancel = lua.create_function(request_run_cancel)?;
    lua.load(
        r#"
        local call_start, batch_start, finish_run, run_is_finished,
            request_run_cancel = ...
        local fiber = require('fiber')

        local function await(handle)
            handle:wait()
            return handle:take()
        end

        local function call(...)
            return await(call_start(...))
        end

        local function batch(...)
            return await(batch_start(...))
        end

        local function join(handle)
            while not run_is_finished(handle) do
                fiber.yield()
            end
            return finish_run(handle)
        end

        local function cancel(handle)
            request_run_cancel(handle)
            return join(handle)
        end

        return call, batch, join, cancel
        "#,
    )
    .call((
        call_start,
        batch_start,
        finish_run,
        run_is_finished,
        request_run_cancel,
    ))
}

/// Execute source code in a module loaded with `{ interpreter = true }`.
fn interpreter_exec(_: &Lua, args: LuaMultiValue) -> LuaResult<bool> {
    let mut iter = args.into_iter();
    let module_uid = parse_required_string(iter.next(), "expected module_uid as string")?;
    let code = parse_required_string(iter.next(), "expected code as string")?;

    tt_wasm_interpreter_exec(module_uid, code).map_err(mlua::Error::external)?;
    Ok(true)
}

const DIRECT_MSGPACK_STRING_MIN_BYTES: usize = 64 * 1024;

fn encode_single_msgpack_string(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < DIRECT_MSGPACK_STRING_MIN_BYTES {
        return None;
    }
    let payload_len = u32::try_from(bytes.len()).ok()?;
    let mut encoded = Vec::with_capacity(6 + bytes.len());
    encoded.extend_from_slice(&[0x91, 0xdb]);
    encoded.extend_from_slice(&payload_len.to_be_bytes());
    encoded.extend_from_slice(bytes);
    Some(encoded)
}

fn encode_interpreter_args(lua: &Lua, args: &[Value]) -> LuaResult<Vec<u8>> {
    if let [Value::String(value)] = args {
        let bytes = value.as_bytes();
        if bytes.len() >= DIRECT_MSGPACK_STRING_MIN_BYTES {
            value.to_str()?;
        }
        if let Some(encoded) = encode_single_msgpack_string(bytes.as_ref()) {
            return Ok(encoded);
        }
    }

    let rmp_args = args
        .iter()
        .map(|value| to_rmpv(lua, value))
        .collect::<Result<Vec<_>, _>>()?;
    rmpv_to_msgpack_bytes(&rmpv::Value::Array(rmp_args)).map_err(mlua::Error::external)
}

fn msgpack_string_payload(bytes: &[u8]) -> Option<&[u8]> {
    let (&marker, rest) = bytes.split_first()?;
    let (header_len, payload_len): (usize, usize) = match marker {
        0xa0..=0xbf => (1, usize::from(marker & 0x1f)),
        0xc4 | 0xd9 => (2, usize::from(*rest.first()?)),
        0xc5 | 0xda => (
            3,
            usize::from(u16::from_be_bytes(rest.get(..2)?.try_into().ok()?)),
        ),
        0xc6 | 0xdb => (
            5,
            usize::try_from(u32::from_be_bytes(rest.get(..4)?.try_into().ok()?)).ok()?,
        ),
        _ => return None,
    };
    (bytes.len() == header_len.checked_add(payload_len)?).then(|| &bytes[header_len..])
}

/// Call a function in a module loaded with `{ interpreter = true }`.
fn interpreter_call(lua: &Lua, args: LuaMultiValue) -> LuaResult<Value> {
    let mut iter = args.into_iter();
    let module_uid = parse_required_string(iter.next(), "expected module_uid as string")?;
    let func = parse_required_string(iter.next(), "expected function name as string")?;
    let rest_args: Vec<Value> = iter.collect();
    let args_msgpack = encode_interpreter_args(lua, &rest_args)?;

    let result_msgpack =
        tt_wasm_interpreter_call(module_uid, func, args_msgpack).map_err(mlua::Error::external)?;
    if result_msgpack.len() >= DIRECT_MSGPACK_STRING_MIN_BYTES {
        if let Some(payload) = msgpack_string_payload(&result_msgpack) {
            return lua.create_string(payload).map(Value::String);
        }
    }
    let result_val = msgpack_bytes_to_rmpv(&result_msgpack).map_err(mlua::Error::external)?;
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
    let (call, batch, join, cancel) = create_async_execution_functions(lua)?;
    let (serialized_interpreter_exec, serialized_interpreter_call) =
        create_interpreter_functions(lua)?;
    let bind = create_bind_function(
        lua,
        serialized_interpreter_exec.clone(),
        serialized_interpreter_call.clone(),
    )?;
    module.set("load", lua.create_function(load)?)?;
    module.set("stats", lua.create_function(stats)?)?;
    module.set("precompile", lua.create_function(precompile)?)?;
    module.set("run", lua.create_function(run)?)?;
    let pipe = lua.create_table()?;
    pipe.set("spawn", lua.create_function(pipe_spawn)?)?;
    module.set("pipe", pipe)?;
    module.set("open_session", lua.create_function(open_session)?)?;
    module.set("join", join)?;
    module.set("cancel", cancel)?;
    module.set("call", call)?;
    module.set("batch", batch)?;
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

#[cfg(test)]
mod msgpack_string_payload_tests {
    use super::*;

    fn encoded(marker: u8, length: &[u8], payload_len: usize) -> Vec<u8> {
        let encoded_len = 1 + length.len() + payload_len;
        let mut bytes = Vec::with_capacity(encoded_len);
        bytes.push(marker);
        bytes.extend_from_slice(length);
        bytes.resize(encoded_len, b'x');
        bytes
    }

    #[test]
    fn accepts_exact_direct_decode_threshold() {
        let bytes = encoded(0xc5, &u16::to_be_bytes(65_533), 65_533);
        assert_eq!(bytes.len(), DIRECT_MSGPACK_STRING_MIN_BYTES);
        assert_eq!(
            msgpack_string_payload(&bytes).map(<[u8]>::len),
            Some(65_533)
        );
    }

    #[test]
    fn encodes_large_single_string_argument() {
        let payload = vec![b'x'; DIRECT_MSGPACK_STRING_MIN_BYTES];
        let encoded = encode_single_msgpack_string(&payload).unwrap();
        assert_eq!(&encoded[..6], &[0x91, 0xdb, 0, 1, 0, 0]);
        assert_eq!(&encoded[6..], payload);
        assert_eq!(
            encode_single_msgpack_string(&payload[..payload.len() - 1]),
            None
        );
    }

    #[test]
    fn accepts_str_and_bin_length_transitions() {
        for (marker, length) in [(0xc5, 65_535_u32), (0xda, 65_535)] {
            let bytes = encoded(marker, &(length as u16).to_be_bytes(), length as usize);
            assert_eq!(
                msgpack_string_payload(&bytes).map(<[u8]>::len),
                Some(65_535)
            );
        }
        for marker in [0xc6, 0xdb] {
            let bytes = encoded(marker, &u32::to_be_bytes(65_536), 65_536);
            assert_eq!(
                msgpack_string_payload(&bytes).map(<[u8]>::len),
                Some(65_536)
            );
        }
    }

    #[test]
    fn rejects_truncated_or_trailing_data() {
        for bytes in [
            &[0xc5, 0][..],
            &[0xc4, 2, b'x'][..],
            &[0xa1, b'x', b'y'][..],
            &[0xc6, 0, 1, 0, 0][..],
        ] {
            assert_eq!(msgpack_string_payload(bytes), None);
        }
    }
}
