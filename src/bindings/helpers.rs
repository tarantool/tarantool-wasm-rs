use crate::bindings::tarantool::tarantool::types::BoxError;
use crate::bindings::types::{validate_msgpack_array, validate_msgpack_map};

use tarantool::error::TarantoolErrorCode;

pub fn illegal_params(message: impl Into<String>) -> BoxError {
    BoxError::new(TarantoolErrorCode::IllegalParams as u32, message)
}

pub fn unknown_error(message: impl Into<String>) -> BoxError {
    BoxError::new(TarantoolErrorCode::Unknown as u32, message)
}

pub fn state_lock_poisoned() -> BoxError {
    unknown_error("state lock poisoned")
}

pub fn validate_array(data: &[u8]) -> Result<(), BoxError> {
    validate_msgpack_array(data).map_err(illegal_params)
}

pub fn validate_map(data: &[u8]) -> Result<(), BoxError> {
    validate_msgpack_map(data).map_err(illegal_params)
}