use crate::bindings::tarantool::tarantool::error;
use crate::bindings::tarantool::tarantool::types::BoxError;
use crate::tarantool_wasm::SyncWasmStates;

use wasmtime_wasi::async_trait;
use std::ffi::CString;

pub fn handle_tarantool_error() -> BoxError {
    let err = tarantool::error::BoxError::last();

    let mut boxed = BoxError::new(
        err.message(),
        err.error_type(),
        err.error_code(),
    );

    if let Some(line) = err.line() {
        boxed = boxed.with_line(line);
    }

    if let Some(file) = err.file() {
        boxed = boxed.with_file(file);
    }

    boxed
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

    pub fn _with_payload(mut self, payload: Vec<(impl Into<String>, impl Into<String>)>) -> Self {
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
        let message_cstr = CString::new(err.message.clone())
            .expect("Failed to convert error message to CString");
    
        let file_line = match (err.file.as_ref(), err.line) {
            (Some(file), Some(line)) => Some((file.as_str(), line)),
            _ => None,
        };
    
        tarantool::error::set_last_error(file_line, err.code, &message_cstr);
    }

    async fn last(&mut self) -> Option<BoxError> {
        match tarantool::error::BoxError::maybe_last() {
            Err(err) => {
                let mut boxed = BoxError::new(
                    err.message(),
                    err.error_type(),
                    err.error_code(),
                );
    
                if let Some(line) = err.line() {
                    boxed = boxed.with_line(line);
                }
    
                if let Some(file) = err.file() {
                    boxed = boxed.with_file(file);
                }
    
                Some(boxed)
            }
            Ok(_) => None,
        }
    }

    async fn clear(&mut self) {
        tarantool::error::clear_error();
    }

    async fn to_string(&mut self, err: BoxError) -> String {
        let mut output = format!("[{}] {}: {}", err.code, err.type_, err.message);
    
        if let (Some(file), Some(line)) = (err.file.as_ref(), err.line) {
            output.push_str(&format!(" (at {}:{})", file, line));
        } else if let Some(file) = err.file.as_ref() {
            output.push_str(&format!(" (in file {})", file));
        } else if let Some(line) = err.line {
            output.push_str(&format!(" (at line {})", line));
        }
    
        if let Some(ref payload) = err.payload {
            output.push_str("\nPayload:");
            for (k, v) in payload {
                output.push_str(&format!("\n  - {}: {}", k, v));
            }
        }
    
        output
    }
}
