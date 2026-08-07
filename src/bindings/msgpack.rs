//! Pure MessagePack/JSON conversion helpers operating on guest-owned buffers.

use crate::bindings::helpers::illegal_params;
use crate::bindings::tarantool::tarantool::msgpack;
use crate::bindings::tarantool::tarantool::types::BoxError;
use crate::bindings::types::validate_msgpack_value;
use crate::runtime::tarantool_wasm::SyncWasmStates;

use rmp_serde::{from_slice, to_vec};
use serde_json::Value;

impl msgpack::Host for SyncWasmStates {
    async fn from_json(&mut self, json: String) -> Result<Vec<u8>, BoxError> {
        let deserialized: Value = match serde_json::from_str(&json) {
            Ok(val) => val,
            Err(e) => return Err(illegal_params(e.to_string())),
        };

        match to_vec(&deserialized) {
            Ok(msgpack) => Ok(msgpack),
            Err(e) => Err(illegal_params(e.to_string())),
        }
    }

    async fn to_json(&mut self, data: Vec<u8>) -> Result<String, BoxError> {
        validate_msgpack_value(&data).map_err(illegal_params)?;
        let deserialized: Value = match from_slice(&data) {
            Ok(val) => val,
            Err(e) => return Err(illegal_params(e.to_string())),
        };

        match serde_json::to_string(&deserialized) {
            Ok(json) => Ok(json),
            Err(e) => Err(illegal_params(e.to_string())),
        }
    }
}
