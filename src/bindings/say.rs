//! Tarantool logging supports native worker threads: `cord()` creates an
//! on-demand thread-local cord, formatting uses a thread-local buffer, logger
//! configuration is mutex-protected, and the final write is thread-safe.
//! These calls intentionally stay off the TX executor.

use crate::bindings::tarantool::tarantool::say;
use crate::bindings::tarantool::tarantool::types::{LogContext, LogLevel};
use crate::runtime::tarantool_wasm::SyncWasmStates;

use tarantool::log::{self, SayLevel};

impl From<LogLevel> for SayLevel {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::SFatal => SayLevel::Fatal,
            LogLevel::SSyserror => SayLevel::System,
            LogLevel::SError => SayLevel::Error,
            LogLevel::SCrit => SayLevel::Crit,
            LogLevel::SWarn => SayLevel::Warn,
            LogLevel::SInfo => SayLevel::Info,
            LogLevel::SVerbose => SayLevel::Verbose,
            LogLevel::SDebug => SayLevel::Debug,
        }
    }
}

fn resolve_context(ctx: Option<LogContext>) -> (String, i32) {
    match ctx {
        Some(context) => (context.filename, context.line as i32),
        None => (file!().to_string(), line!() as i32),
    }
}

impl say::Host for SyncWasmStates {
    async fn say(&mut self, level: LogLevel, msg: String, ctx: Option<LogContext>) {
        let log_level: SayLevel = level.into();
        let (f, l) = resolve_context(ctx);
        log::say(log_level, &f, l, None, &msg);
    }

    async fn say_error(&mut self, msg: String, ctx: Option<LogContext>) {
        let (f, l) = resolve_context(ctx);
        log::say(SayLevel::Error, &f, l, None, &msg);
    }

    async fn say_crit(&mut self, msg: String, ctx: Option<LogContext>) {
        let (f, l) = resolve_context(ctx);
        log::say(SayLevel::Crit, &f, l, None, &msg);
    }

    async fn say_warn(&mut self, msg: String, ctx: Option<LogContext>) {
        let (f, l) = resolve_context(ctx);
        log::say(SayLevel::Warn, &f, l, None, &msg);
    }

    async fn say_info(&mut self, msg: String, ctx: Option<LogContext>) {
        let (f, l) = resolve_context(ctx);
        log::say(SayLevel::Info, &f, l, None, &msg);
    }

    async fn say_verbose(&mut self, msg: String, ctx: Option<LogContext>) {
        let (f, l) = resolve_context(ctx);
        log::say(SayLevel::Verbose, &f, l, None, &msg);
    }

    async fn say_debug(&mut self, msg: String, ctx: Option<LogContext>) {
        let (f, l) = resolve_context(ctx);
        log::say(SayLevel::Debug, &f, l, None, &msg);
    }

    async fn say_syserror(&mut self, msg: String, ctx: Option<LogContext>) {
        let (f, l) = resolve_context(ctx);
        log::say(SayLevel::System, &f, l, None, &msg);
    }
}
