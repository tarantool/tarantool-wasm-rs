use crate::bindings::tarantool::tarantool::session;
use crate::bindings::tarantool::tarantool::types::{BoxError, Session};
use crate::tarantool_wasm::SyncWasmStates;
use crate::ffi as ffi;
use crate::bindings::error::handle_tarantool_error;
use crate::bindings::types::validate_msgpack_array;
use sha2::{Digest, Sha256};



impl session::Host for SyncWasmStates {

    async fn current(&mut self) -> Session {
        let session_id = unsafe { ffi::session::box_session_id() };
        return Session {
            id: session_id as i64,
        }
    }

    async fn iproto_send(&mut self, session: Session, header: Vec<u8>, body: Vec<u8>) -> Result<(), BoxError> {
        if let Err(e) = validate_msgpack_array(&header) {
            return Err(BoxError::new(&e, "ValidationError", 1));
        }

        if let Err(e) = validate_msgpack_array(&body) {
            return Err(BoxError::new(&e, "ValidationError", 1));
        }

        let header_copy = header.clone();
        let body_copy = body.clone();
    
        self.executor().execute(move || {
            let header_start = header_copy.as_ptr();
            let header_end = unsafe { header_start.add(header_copy.len()) };
            let body_start = body_copy.as_ptr();
            let body_end = unsafe { body_start.add(body_copy.len()) };

            let rc = unsafe {
                ffi::session::box_iproto_send(session.id as u64, header_start, header_end, body_start, body_end)
            };
            if rc != 0 {
                return Err(handle_tarantool_error())
            }
            Ok(())
        }).await
    }

    async fn broadcast(&mut self, key: String, value: String) -> Result<(), BoxError> {
    
        if key.is_empty() {
            return Err(BoxError::new("Key is empty", "ValidationError", 1));
        }
        if value.is_empty() {
            return Err(BoxError::new("Value is empty", "ValidationError", 1));
        }
    
        let key_copy = key.clone();
        let value_copy = value.clone();

        self.executor().execute(move || {
            let mut hasher = Sha256::new();
            hasher.update(value_copy.as_bytes());
            let hash = hasher.finalize();
            let hash_str = format!("{:x}", hash);
            let var_name = format!("__wasm_{}", hash_str);
            let lua = tarantool::lua_state();
            lua.exec_with(&format!("{} = {};", var_name, value_copy), ()).unwrap();
            let broadcast_result = lua.exec_with(&format!("box.broadcast(..., {})", var_name), key_copy);

            if let Err(err) = broadcast_result {
                return Err(BoxError::new(format!("Lua error during broadcast: {:?}", err), "LuaError", 1))
            }
            Ok(())
        }).await
    }
}
