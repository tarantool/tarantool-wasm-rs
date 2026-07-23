//! Host calls from WebAssembly components into registered Lua closures.

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use crate::bindings::docs::adder::lua_callback;
use crate::bindings::helpers::debug_assert_tx_thread;
use crate::bindings::LuaCallbackHandler;
use crate::runtime::convert::{from_rmpv, msgpack_bytes_to_rmpv, rmpv_to_msgpack_bytes, to_rmpv};
use crate::runtime::lifecycle::{self, RuntimeLifecycle};
use crate::runtime::metrics;
use crate::runtime::tarantool_wasm::install_call_chain;
use crate::runtime::tarantool_wasm::SyncWasmStates;
use mlua::prelude::LuaThreadStatus;
use mlua::{HookTriggers, VmState};
use tarantool::tlua::AsLua;
use wasmtime::component::Resource;

pub(crate) const CALLBACK_REGISTRY_PREFIX: &str = "tarantool-wasm-rs.callback.";

pub(crate) fn callback_registry_key(name: &str) -> String {
    format!("{CALLBACK_REGISTRY_PREFIX}{name}")
}

struct CallbackCancellation {
    canceled: Arc<AtomicBool>,
}

impl Drop for CallbackCancellation {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
    }
}

#[derive(Clone)]
struct CallbackControl {
    name: String,
    canceled: Arc<AtomicBool>,
    deadline: Option<Instant>,
}

impl CallbackControl {
    fn check(&self) -> mlua::Result<()> {
        if self.canceled.load(Ordering::Acquire) || lifecycle::state() != RuntimeLifecycle::Running
        {
            return Err(mlua::Error::runtime(format!(
                "Lua callback `{}` was canceled",
                self.name
            )));
        }
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err(mlua::Error::runtime(format!(
                "Lua callback `{}` timed out",
                self.name
            )));
        }
        Ok(())
    }
}

fn callback_registration(
    lua: &mlua::Lua,
    name: &str,
) -> Result<(mlua::Function, Option<Duration>), String> {
    let key = callback_registry_key(name);
    let registration: mlua::Value = lua
        .named_registry_value(&key)
        .map_err(|_| format!("Lua callback `{name}` is not registered"))?;
    match registration {
        // Accept registrations created before this runtime version was
        // reloaded in the same Tarantool process.
        mlua::Value::Function(callback) => Ok((callback, None)),
        mlua::Value::Table(registration) => {
            let callback = registration
                .get("callback")
                .map_err(|err| format!("invalid Lua callback `{name}` registration: {err}"))?;
            let timeout_ms: Option<u64> = registration
                .get("timeout_ms")
                .map_err(|err| format!("invalid Lua callback `{name}` timeout: {err}"))?;
            Ok((callback, timeout_ms.map(Duration::from_millis)))
        }
        _ => Err(format!("Lua callback `{name}` is not registered")),
    }
}

fn invoke_callback(
    lua: &mlua::Lua,
    name: String,
    payload: Vec<u8>,
    canceled: Arc<AtomicBool>,
) -> Result<Vec<u8>, String> {
    let started = Instant::now();
    let result = invoke_callback_inner(lua, name, payload, canceled);
    metrics::observe_lua_callback(started.elapsed());
    result
}

fn invoke_callback_inner(
    lua: &mlua::Lua,
    name: String,
    payload: Vec<u8>,
    canceled: Arc<AtomicBool>,
) -> Result<Vec<u8>, String> {
    let (callback, timeout) = callback_registration(lua, &name)?;
    let request = msgpack_bytes_to_rmpv(&payload)
        .inspect_err(|_| metrics::observe_conversion_error("callback_request"))
        .map_err(|err| format!("invalid MessagePack callback payload: {err:#}"))?;
    let lua_request = from_rmpv(lua, &request)
        .inspect_err(|_| metrics::observe_conversion_error("callback_request"))
        .map_err(|err| format!("failed to convert callback payload to Lua: {err}"))?;

    let deadline = timeout
        .map(|timeout| {
            Instant::now()
                .checked_add(timeout)
                .ok_or_else(|| format!("Lua callback `{name}` timeout is too large"))
        })
        .transpose()?;
    let control = CallbackControl {
        name: name.clone(),
        canceled,
        deadline,
    };
    control
        .check()
        .map_err(|err| format!("Lua callback `{name}` failed: {err}"))?;

    let thread = lua
        .create_thread(callback)
        .map_err(|err| format!("failed to create Lua callback `{name}` coroutine: {err}"))?;
    let hook_control = control.clone();
    thread
        .set_hook(
            HookTriggers::new().every_nth_instruction(10_000),
            move |_, _| {
                hook_control.check()?;
                Ok(VmState::Continue)
            },
        )
        .map_err(|err| format!("failed to configure Lua callback `{name}` coroutine: {err}"))?;

    let mut lua_response: mlua::Value = thread
        .resume(lua_request)
        .map_err(|err| format!("Lua callback `{name}` failed: {err}"))?;
    loop {
        control
            .check()
            .map_err(|err| format!("Lua callback `{name}` failed: {err}"))?;
        if thread.status() != LuaThreadStatus::Resumable {
            break;
        }
        lua_response = thread
            .resume(())
            .map_err(|err| format!("Lua callback `{name}` failed: {err}"))?;
    }

    let response = to_rmpv(lua, &lua_response)
        .inspect_err(|_| metrics::observe_conversion_error("callback_response"))
        .map_err(|err| format!("failed to convert Lua callback response: {err}"))?;
    rmpv_to_msgpack_bytes(&response)
        .inspect_err(|_| metrics::observe_conversion_error("callback_response"))
        .map_err(|err| format!("failed to encode Lua callback response: {err:#}"))
}

impl lua_callback::Host for SyncWasmStates {
    async fn open(&mut self, name: String) -> Result<Resource<LuaCallbackHandler>, String> {
        if name.is_empty() {
            return Err("callback name must not be empty".to_string());
        }

        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        state
            .table
            .push(LuaCallbackHandler { name })
            .map_err(|err| format!("failed to create Lua callback handler: {err}"))
    }
}

impl lua_callback::HostHandler for SyncWasmStates {
    async fn call(
        &mut self,
        self_: Resource<LuaCallbackHandler>,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        let name = {
            let state = self
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            state
                .table
                .get(&self_)
                .map_err(|err| format!("invalid Lua callback handler: {err}"))?
                .name
                .clone()
        };
        let call_chain = self.call_chain();
        let canceled = Arc::new(AtomicBool::new(false));
        let _cancel_on_drop = CallbackCancellation {
            canceled: Arc::clone(&canceled),
        };

        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let _call_chain_guard = install_call_chain(call_chain);
                let lua_thread = tarantool::lua_state();
                // SAFETY: execution is pinned to Tarantool's TX thread and
                // the thread handle keeps the underlying state alive for this call.
                let state = lua_thread.as_lua().cast::<mlua::ffi::lua_State>();
                let lua = unsafe { mlua::Lua::get_or_init_from_ptr(state) };
                invoke_callback(lua, name, payload, canceled)
            })
            .await
    }

    async fn drop(&mut self, self_: Resource<LuaCallbackHandler>) -> Result<(), wasmtime::Error> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        state.table.delete(self_)?;
        Ok(())
    }
}
