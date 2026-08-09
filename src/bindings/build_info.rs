use crate::bindings::docs::adder::build_info;
use crate::ffi::raw_read_view;
use crate::runtime::tarantool_wasm::SyncWasmStates;

pub(crate) fn is_enterprise() -> bool {
    raw_read_view::is_available()
}

impl build_info::Host for SyncWasmStates {
    async fn current_edition(&mut self) -> build_info::Edition {
        if is_enterprise() {
            build_info::Edition::Enterprise
        } else {
            build_info::Edition::Community
        }
    }
}
