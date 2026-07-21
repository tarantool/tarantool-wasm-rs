use crate::bindings::tarantool::tarantool::types::BoxError;
use crate::bindings::types::{validate_msgpack_array, validate_msgpack_map};

use tarantool::error::TarantoolErrorCode;

/// Assert that a Tarantool API requiring TX state is called from the TX thread.
///
/// The check is enabled in debug builds, including the regular CI test build.
#[inline]
pub fn debug_assert_tx_thread() {
    crate::runtime::tarantool_wasm::debug_assert_tx_thread();
}

/// Restores Tarantool's fiber-local box region when a C API returns data
/// allocated there.
pub struct BoxRegionGuard {
    savepoint: usize,
}

impl BoxRegionGuard {
    pub fn new() -> Self {
        debug_assert_tx_thread();
        Self {
            savepoint: unsafe { crate::ffi::r#box::box_region_used() },
        }
    }
}

impl Default for BoxRegionGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for BoxRegionGuard {
    fn drop(&mut self) {
        debug_assert_tx_thread();
        unsafe { crate::ffi::r#box::box_region_truncate(self.savepoint) };
    }
}

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
