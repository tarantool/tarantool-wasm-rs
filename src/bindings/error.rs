use crate::bindings::helpers::debug_assert_tx_thread;
use crate::bindings::tarantool::tarantool::error;
use crate::bindings::tarantool::tarantool::types::{BoxError, ErrorCode};
use crate::runtime::tarantool_wasm::SyncWasmStates;

use tarantool::error::{self as tnt_error, TarantoolErrorCode};
use tarantool::util::into_cstring_lossy;

pub fn handle_tarantool_error() -> BoxError {
    debug_assert_tx_thread();
    let err = tarantool::error::BoxError::last();

    let mut boxed = BoxError::new(err.error_code(), err.message());

    if let Some(line) = err.line() {
        boxed = boxed.with_line(line);
    }

    if let Some(file) = err.file() {
        boxed = boxed.with_file(file);
    }

    boxed
}

pub fn map_tarantool_error(err: tnt_error::Error) -> BoxError {
    match err {
        tnt_error::Error::Tarantool(_) => handle_tarantool_error(),
        other => BoxError::new(TarantoolErrorCode::Unknown as u32, other.to_string()),
    }
}

impl BoxError {
    pub fn new(code: u32, message: impl Into<String>) -> Self {
        let msg: String = message.into();
        let tmp = tarantool::error::BoxError::new(code, &msg);
        let error_type = tmp.error_type().to_string();

        let file = None;
        let line = None;

        Self {
            code,
            message: msg,
            error_type,
            file,
            line,
        }
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
    async fn new(&mut self, code: ErrorCode, message: String) -> BoxError {
        BoxError::new(code, message)
    }

    async fn new_with_location(
        &mut self,
        code: ErrorCode,
        message: String,
        file: Option<String>,
        line: Option<u32>,
    ) -> BoxError {
        let mut err = BoxError::new(code, message);

        if let Some(file) = file {
            err = err.with_file(file);
        }
        if let Some(line) = line {
            err = err.with_line(line);
        }

        err
    }

    async fn set(&mut self, err: BoxError) {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let BoxError {
                    code,
                    message,
                    file,
                    line,
                    ..
                } = err;
                let message_cstr = into_cstring_lossy(message);
                let file_line = match (file.as_deref(), line) {
                    (Some(file), Some(line)) => Some((file, line)),
                    _ => None,
                };
                tarantool::error::set_last_error(file_line, code, &message_cstr);
            })
            .await;
    }

    async fn last(&mut self) -> Option<BoxError> {
        self.executor()
            .execute(|| {
                debug_assert_tx_thread();
                match tarantool::error::BoxError::maybe_last() {
                    Err(err) => {
                        let mut boxed = BoxError::new(err.error_code(), err.message());
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
            })
            .await
    }

    async fn clear(&mut self) {
        self.executor()
            .execute(|| {
                debug_assert_tx_thread();
                tarantool::error::clear_error();
            })
            .await;
    }

    async fn to_string(&mut self, err: BoxError) -> String {
        let mut output = format!("[{}] {}: {}", err.code, err.error_type, err.message);

        if let (Some(file), Some(line)) = (err.file.as_ref(), err.line) {
            output.push_str(&format!(" (at {file}:{line})"));
        } else if let Some(file) = err.file.as_ref() {
            output.push_str(&format!(" (in file {file})"));
        } else if let Some(line) = err.line {
            output.push_str(&format!(" (at line {line})"));
        }

        output
    }
}
