//! Tarantool logging supports native worker threads: `cord()` creates an
//! on-demand thread-local cord, formatting uses a thread-local buffer, logger
//! configuration is mutex-protected, and the final write is thread-safe.
//! These calls intentionally stay off the TX executor.

use crate::bindings::tarantool::tarantool::log;
use crate::bindings::tarantool::tarantool::types::{LogContext, LogLevel};
use crate::runtime::tarantool_wasm::SyncWasmStates;

use tarantool::log::{self as tnt_log, SayLevel};

impl From<LogLevel> for SayLevel {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => SayLevel::Error,
            LogLevel::Critical => SayLevel::Crit,
            LogLevel::Warning => SayLevel::Warn,
            LogLevel::Info => SayLevel::Info,
            LogLevel::Verbose => SayLevel::Verbose,
            LogLevel::Debug => SayLevel::Debug,
        }
    }
}

fn resolve_context(ctx: Option<LogContext>) -> (String, i32) {
    match ctx {
        Some(context) => (context.file, context.line as i32),
        None => (file!().to_string(), line!() as i32),
    }
}

impl log::Host for SyncWasmStates {
    async fn write(&mut self, level: LogLevel, msg: String, ctx: Option<LogContext>) {
        let log_level: SayLevel = level.into();
        let (f, l) = resolve_context(ctx);
        tnt_log::say(log_level, &f, l, None, &msg);
    }
}
