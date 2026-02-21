use crate::bindings::error::handle_tarantool_error;
use crate::bindings::tarantool::tarantool::session;
use crate::bindings::tarantool::tarantool::types::{BoxError, Session};
use crate::bindings::types::validate_msgpack_map;
use crate::ffi;
use crate::runtime::tarantool_wasm::SyncWasmStates;

use tarantool::error::TarantoolErrorCode;

impl session::Host for SyncWasmStates {
    async fn current(&mut self) -> Session {
        let session_id = unsafe { ffi::session::box_session_id() };
        Session {
            id: session_id as i64,
        }
    }

    async fn iproto_send(
        &mut self,
        session: Session,
        header: Vec<u8>,
        body: Vec<u8>,
    ) -> Result<(), BoxError> {
        if let Err(e) = validate_msgpack_map(&header) {
            return Err(BoxError::new(TarantoolErrorCode::IllegalParams as u32, &e));
        }

        self.executor()
            .execute(move || {
                let header_start = header.as_ptr();
                let header_end = unsafe { header_start.add(header.len()) };
                let body_start = body.as_ptr();
                let body_end = unsafe { body_start.add(body.len()) };

                let rc = unsafe {
                    ffi::session::box_iproto_send(
                        session.id as u64,
                        header_start,
                        header_end,
                        body_start,
                        body_end,
                    )
                };
                if rc != 0 {
                    return Err(handle_tarantool_error());
                }
                Ok(())
            })
            .await
    }

    async fn broadcast(&mut self, key: String, value: String) -> Result<(), BoxError> {
        if key.is_empty() {
            return Err(BoxError::new(
                TarantoolErrorCode::IllegalParams as u32,
                "Key is empty",
            ));
        }
        if value.is_empty() {
            return Err(BoxError::new(
                TarantoolErrorCode::IllegalParams as u32,
                "Value is empty",
            ));
        }

        self.executor()
            .execute(move || {
                let lua = tarantool::lua_state();
                let broadcast_result = lua.exec_with(
                    r#"
                        local key, expr = ...
                        local chunk, err = load("return " .. expr)
                        if chunk == nil then
                            error(err)
                        end
                        local ok, payload = pcall(chunk)
                        if not ok then
                            error(payload)
                        end
                        box.broadcast(key, payload)
                    "#,
                    (key, value),
                );

                if let Err(err) = broadcast_result {
                    return Err(BoxError::new(
                        TarantoolErrorCode::Unknown as u32,
                        format!("Lua error during broadcast: {err:?}"),
                    ));
                }
                Ok(())
            })
            .await
    }
}
