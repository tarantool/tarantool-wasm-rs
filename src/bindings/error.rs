use crate::bindings::docs::adder::error;
use crate::bindings::docs::adder::types::BoxError;
use crate::tarantool_wasm::SyncWasmStates;

use wasmtime_wasi::async_trait;
use tarantool::error::TarantoolError;
use std::ffi::{CStr, CString};

extern "C" {
    fn box_error_last() -> *const TarantoolError;
    fn box_error_type(error: *const TarantoolError) -> *const i8;
    fn box_error_code(error: *const TarantoolError) -> u32;
    fn box_error_message(error: *const TarantoolError) -> *const i8;
    fn box_error_clear();
    fn box_error_set(file: *const i8, line: u32, code: u32, format: *const i8, ...);
}

#[async_trait]
impl error::Host for SyncWasmStates {
    async fn new(&mut self, message: String, type_: String, code: u32) -> BoxError {
        BoxError {
            message: message,
            type_: type_,
            code: code,
            payload: None,
            file: None,
            line: None,
        }
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
            box_error_set(file.as_ptr(), 0, err.code, format.as_ptr());
        }

        println!("RS | Error set successfully");
    }

    async fn last(&mut self) -> Option<BoxError> {
        unsafe {
            let err_ptr = box_error_last();
            if err_ptr.is_null() {
                return None;
            }

            let message = CStr::from_ptr(box_error_message(err_ptr))
                .to_string_lossy()
                .into_owned();
            let type_ = CStr::from_ptr(box_error_type(err_ptr))
                .to_string_lossy()
                .into_owned();
            let code = box_error_code(err_ptr);

            Some(BoxError {
                    message: message,
                    type_: type_,
                    code: code,
                    payload: None,
                    file: None,
                    line: None,
            })
        }
    }

    async fn clear(&mut self) {
        unsafe {
            box_error_clear();
        }
    }

    async fn to_string(&mut self, err: BoxError) -> String {
        format!("[{}] {}: {}", err.code, err.type_, err.message)
    }
}
