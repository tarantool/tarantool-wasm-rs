use crate::bindings::docs::adder::msgpack;
use crate::tarantool_wasm::SyncWasmStates;
use crate::bindings::docs::adder::types::BoxError;

use wasmtime_wasi::async_trait;
use rmp_serde::{from_slice, to_vec};
use serde_json::Value;
use std::io::Cursor;
use libc;
use rmpv::decode::read_value;

#[async_trait]
impl msgpack::Host for SyncWasmStates {

    async fn encode(&mut self, data: Vec<u8>) -> Result<Vec<u8>, BoxError> {
        let deserialized: Value = match serde_json::from_slice(&data) {
            Ok(val) => val,
            Err(e) => return Err(BoxError::new(e.to_string(), "JsonError", 1)),
        };

        match to_vec(&deserialized) {
            Ok(msgpack) => Ok(msgpack),
            Err(e) => return Err(BoxError::new(e.to_string(), "MsgpackError", 1)),
        }
    }

    async fn decode(&mut self, data: Vec<u8>) -> Result<Vec<u8>, BoxError> {
        let deserialized: Value = match from_slice(&data) {
            Ok(val) => val,
            Err(e) => return Err(BoxError::new(e.to_string(), "MsgpackError", 1)),

        };

        match serde_json::to_vec(&deserialized) {
            Ok(json_bytes) => Ok(json_bytes),
            Err(e) => return Err(BoxError::new(e.to_string(), "JsonError", 1)),
        }
    }

    async fn decode_from_raw_ptr(&mut self, ptr: u64) -> Result<Vec<u8>, BoxError> {

        if ptr == 0 {
            return Err(BoxError::new("Null pointer provided", "DecodeError", 500))
        }

        let data_ptr = ptr as *const libc::c_char;
        const MAX_MSGPACK_SIZE: usize = 65536;

        // MsgPack doesn't have a terminator symbol, so we rely on rmpv to parse it
        // First, wrap raw pointer in unsafe slice
        // We must carefully handle this to avoid reading invalid memory.
        let raw_slice = unsafe { std::slice::from_raw_parts(data_ptr as *const u8, MAX_MSGPACK_SIZE) };
        let mut cursor = Cursor::new(raw_slice);

        match read_value(&mut cursor) {
            Ok(value) => {
                let json_bytes = serde_json::to_vec(&value).map_err(|e| {
                    BoxError::new(e.to_string(), "JsonError", 1)
                })?;
                Ok(json_bytes)
            },
            Err(e) => return Err(BoxError::new(e.to_string(), "MsgpackError", 1)),
        }
    }
}
