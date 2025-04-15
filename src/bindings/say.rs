use crate::bindings::tarantool::tarantool::say;
use crate::bindings::tarantool::tarantool::types::LogLevel;
use crate::tarantool_wasm::SyncWasmStates;

use wasmtime_wasi::async_trait;
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

#[async_trait]
impl say::Host for SyncWasmStates {
    async fn say(&mut self, level: LogLevel, msg: String) {
        let log_level: SayLevel = level.into();
        log::say(log_level, file!(), line!() as i32, None, &msg);
    }

    async fn say_error(&mut self, msg: String) {
        log::say(SayLevel::Error, file!(), line!() as i32, None, &msg);
    }

    async fn say_crit(&mut self, msg: String) {
        log::say(SayLevel::Crit, file!(), line!() as i32, None, &msg);
    }

    async fn say_warn(&mut self, msg: String) {
        log::say(SayLevel::Warn, file!(), line!() as i32, None, &msg);
    }

    async fn say_info(&mut self, msg: String) {
        log::say(SayLevel::Info, file!(), line!() as i32, None, &msg);
    }

    async fn say_verbose(&mut self, msg: String) {
        log::say(SayLevel::Verbose, file!(), line!() as i32, None, &msg);
    }

    async fn say_debug(&mut self, msg: String) {
        log::say(SayLevel::Debug, file!(), line!() as i32, None, &msg);
    }

    async fn say_syserror(&mut self, msg: String) {
        log::say(SayLevel::System, file!(), line!() as i32, None, &msg);
    }
}
