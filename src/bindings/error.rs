use crate::bindings::docs::adder::error;
use crate::bindings::docs::adder::types::BoxError;
use crate::tarantool_wasm::SyncWasmStates;
use crate::ffi as ffi;

use wasmtime_wasi::async_trait;
use std::ffi::{CStr, CString};

pub unsafe fn handle_tarantool_error() -> BoxError {
    let err_ptr: *const ffi::types::BoxErrorT = ffi::error::box_error_last();

    if err_ptr.is_null() {
        println!("RS | error: Unknown Tarantool error");
        return BoxError::new("Unknown Tarantool error", "tarantool error", 0);
    }

    let message = CStr::from_ptr(ffi::error::box_error_message(err_ptr))
        .to_string_lossy()
        .into_owned();

    let type_ = CStr::from_ptr(ffi::error::box_error_type(err_ptr))
        .to_string_lossy()
        .into_owned();

    let code = ffi::error::box_error_code(err_ptr);

    println!("RS | error: [{}] {}: {}", code, type_, message);
    BoxError::new(message, type_, code)
}

impl BoxError {
    pub fn new(message: impl Into<String>, type_: impl Into<String>, code: u32) -> Self {
        Self {
            message: message.into(),
            type_: type_.into(),
            code,
            payload: None,
            file: None,
            line: None,
        }
    }

    pub fn with_payload(mut self, payload: Vec<(impl Into<String>, impl Into<String>)>) -> Self {
        self.payload = Some(payload.into_iter().map(|(k, v)| (k.into(), v.into())).collect());
        self
    }

    pub fn with_file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }

    pub fn with_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }
}

#[async_trait]
impl error::Host for SyncWasmStates {
    async fn error_new(&mut self, message: String, type_: String, code: u32) -> BoxError {
        BoxError::new(message, type_, code)
    }

    async fn set(&mut self, err: BoxError) {
        println!("RS | Setting error:");
        println!("RS | - Type: {}", err.type_);
        println!("RS | - Code: {}", err.code);
        println!("RS | - Message: {}", err.message);

        // TODO: replace
        let file = CString::new("rust").unwrap();
        let format = CString::new(err.message.clone()).unwrap();

        unsafe {
            ffi::error::box_error_set(file.as_ptr(), 0, err.code, format.as_ptr());
        }

        println!("RS | Error set successfully");
    }

    async fn last(&mut self) -> Option<BoxError> {
        unsafe { Some(handle_tarantool_error()) }
    }

    async fn clear(&mut self) {
        unsafe {
            ffi::error::box_error_clear();
        }
    }

    async fn to_string(&mut self, err: BoxError) -> String {
        format!("[{}] {}: {}", err.code, err.type_, err.message)
    }
}
