use crate::bindings::docs::adder::msgpack;
use crate::tarantool_wasm::SyncWasmStates;
use crate::bindings::docs::adder::types::BoxError;

use wasmtime_wasi::async_trait;
use rmp_serde::{from_slice, to_vec};
use serde_json::Value;

#[async_trait]
impl msgpack::Host for SyncWasmStates {

    async fn encode(&mut self, data: Vec<u8>) -> Result<Vec<u8>, BoxError> {
        let deserialized: Value = match serde_json::from_slice(&data) {
            Ok(val) => val,
            Err(e) => {
                return Err(BoxError::new(format!("JSON decode error: {:?}", e), "internal error".to_string(), 500));
            }
        };

        match to_vec(&deserialized) {
            Ok(msgpack) => Ok(msgpack),
            Err(e) => Err(BoxError::new(format!("MsgPack encoding error: {:?}", e), "internal error".to_string(), 500)),
        }
    }

    async fn decode(&mut self, data: Vec<u8>) -> Result<Vec<u8>, BoxError> {
        let deserialized: Value = match from_slice(&data) {
            Ok(val) => val,
            Err(e) => {
                return Err(BoxError::new(format!("MsgPack decode error: {:?}", e), "internal error".to_string(), 500));
            }
        };

        match serde_json::to_vec(&deserialized) {
            Ok(json_bytes) => Ok(json_bytes),
            Err(e) => Err(BoxError::new(format!("JSON encode error: {:?}", e), "internal error".to_string(), 500)),
        }
    } 
}

