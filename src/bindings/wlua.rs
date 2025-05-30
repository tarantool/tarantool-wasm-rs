use crate::bindings::docs::adder::wlua;
use crate::tarantool_wasm::SyncWasmStates;


impl wlua::Host for SyncWasmStates {
    async fn call(&mut self, code: String) -> String {
        let lua = tarantool::lua_state();
        let chunk = format!(
            r#"
local __val = ({0})
if type(__val) == 'table' then
    __result = require('yaml').encode(__val)
else
    __result = tostring(__val)
end
"#,
            code
        );
        
        if let Err(err) = lua.exec_with(&chunk, ()) {
            return format!("Lua error: {}", err);
        }

        lua.get("__result").unwrap()
    }
}
