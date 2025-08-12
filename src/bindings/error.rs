use crate::bindings::tarantool::tarantool::error;
use crate::bindings::tarantool::tarantool::types::BoxError;
use crate::runtime::tarantool_wasm::SyncWasmStates;

use std::ffi::CString;
use tarantool::error::{self as tnt_error, TarantoolErrorCode};

pub fn handle_tarantool_error() -> BoxError {
    let err = tarantool::error::BoxError::last();

    let mut boxed = BoxError::new(err.message(), err.error_type(), err.error_code());

    if let Some(line) = err.line() {
        boxed = boxed.with_line(line);
    }

    if let Some(file) = err.file() {
        boxed = boxed.with_file(file);
    }

    if !err.fields().is_empty() {
        let payload = err
            .fields()
            .iter()
            .map(|(k, v)| (k.to_string(), format!("{v}")))
            .collect();
        boxed = boxed.with_payload(payload);
    }

    boxed
}

pub fn map_tarantool_error(err: tnt_error::Error, type_: &str) -> BoxError {
    match err {
        tnt_error::Error::Tarantool(_) => handle_tarantool_error(),
        other => BoxError::new(other.to_string(), type_, TarantoolErrorCode::Unknown as u32),
    }
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
        self.payload = Some(
            payload
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        );
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

impl error::Host for SyncWasmStates {
    async fn error_new(
        &mut self,
        message: String,
        type_: String,
        code: u32,
        file: Option<String>,
        line: Option<u32>,
    ) -> BoxError {
        let mut err = BoxError::new(message, type_, code);
        if let Some(file) = file {
            err = err.with_file(file);
        }
        if let Some(line) = line {
            err = err.with_line(line);
        }
        err
    }

    async fn set(&mut self, err: BoxError) {
        let message_cstr =
            CString::new(err.message.clone()).expect("Failed to convert error message to CString");

        let file_line = match (err.file.as_ref(), err.line) {
            (Some(file), Some(line)) => Some((file.as_str(), line)),
            _ => None,
        };

        tarantool::error::set_last_error(file_line, err.code, &message_cstr);
    }

    async fn last(&mut self) -> Option<BoxError> {
        match tarantool::error::BoxError::maybe_last() {
            Err(err) => {
                let mut boxed = BoxError::new(err.message(), err.error_type(), err.error_code());

                if let Some(line) = err.line() {
                    boxed = boxed.with_line(line);
                }

                if let Some(file) = err.file() {
                    boxed = boxed.with_file(file);
                }

                if !err.fields().is_empty() {
                    let payload = err
                        .fields()
                        .iter()
                        .map(|(k, v)| (k.to_string(), format!("{v}")))
                        .collect();
                    boxed = boxed.with_payload(payload);
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
            output.push_str(&format!(" (at {file}:{line})"));
        } else if let Some(file) = err.file.as_ref() {
            output.push_str(&format!(" (in file {file})"));
        } else if let Some(line) = err.line {
            output.push_str(&format!(" (at line {line})"));
        }

        if let Some(ref payload) = err.payload {
            output.push_str("\nPayload:");
            for (k, v) in payload {
                output.push_str(&format!("\n  - {k}: {v}"));
            }
        }

        output
    }
}
