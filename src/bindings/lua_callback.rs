//! Host calls from WebAssembly components into registered Lua closures.

use crate::bindings::docs::adder::lua_callback;
use crate::bindings::helpers::debug_assert_tx_thread;
use crate::bindings::LuaCallbackHandler;
use crate::runtime::convert::{from_rmpv, msgpack_bytes_to_rmpv, rmpv_to_msgpack_bytes, to_rmpv};
use crate::runtime::tarantool_wasm::SyncWasmStates;
use tarantool::tlua::AsLua;
use wasmtime::component::Resource;

pub(crate) const CALLBACK_REGISTRY_PREFIX: &str = "tarantool-wasm-rs.callback.";

pub(crate) fn callback_registry_key(name: &str) -> String {
    format!("{CALLBACK_REGISTRY_PREFIX}{name}")
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

        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let lua_thread = tarantool::lua_state();
                // SAFETY: execution is pinned to Tarantool's TX thread and
                // the thread handle keeps the underlying state alive for this call.
                let state = lua_thread.as_lua().cast::<mlua::ffi::lua_State>();
                let lua = unsafe { mlua::Lua::get_or_init_from_ptr(state) };
                let key = callback_registry_key(&name);
                let callback: mlua::Function = lua
                    .named_registry_value(&key)
                    .map_err(|_| format!("Lua callback `{name}` is not registered"))?;
                let request = msgpack_bytes_to_rmpv(&payload)
                    .map_err(|err| format!("invalid MessagePack callback payload: {err:#}"))?;
                let lua_request = from_rmpv(lua, &request)
                    .map_err(|err| format!("failed to convert callback payload to Lua: {err}"))?;
                let lua_response: mlua::Value = callback
                    .call(lua_request)
                    .map_err(|err| format!("Lua callback `{name}` failed: {err}"))?;
                let response = to_rmpv(lua, &lua_response)
                    .map_err(|err| format!("failed to convert Lua callback response: {err}"))?;
                rmpv_to_msgpack_bytes(&response)
                    .map_err(|err| format!("failed to encode Lua callback response: {err:#}"))
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
