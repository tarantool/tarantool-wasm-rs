use crate::bindings::tarantool::tarantool::metrics;
use crate::ffi::tarantool_extra::tarantool_uptime;
use crate::runtime::tarantool_wasm::SyncWasmStates;

impl metrics::Host for SyncWasmStates {
    async fn uptime(&mut self) -> f64 {
        self.yield_now().await;
        let res = unsafe { tarantool_uptime() as f64 };
        self.yield_now().await;
        res
    }
}
