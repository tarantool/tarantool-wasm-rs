use crate::bindings::tarantool::tarantool::metrics;
use crate::ffi::tarantool_extra::tarantool_uptime;
use crate::runtime::tarantool_wasm::SyncWasmStates;

impl metrics::Host for SyncWasmStates {
    async fn uptime(&mut self) -> f64 {
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                unsafe { tarantool_uptime() as f64 }
            })
            .await
    }
}
use crate::bindings::helpers::debug_assert_tx_thread;
