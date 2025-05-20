use crate::bindings::tarantool::tarantool::tuple_format;
use crate::bindings::tarantool::tarantool::types::{BoxError, KeyDef};
use crate::tarantool_wasm::SyncWasmStates;
use crate::ffi as ffi;
use crate::bindings::error::handle_tarantool_error;


use tarantool::tuple::TupleFormat as TarantoolTupleFormat;

pub type TupleFormat = u64;

impl tuple_format::Host for SyncWasmStates {
    async fn default(&mut self) -> TupleFormat {
        TarantoolTupleFormat::default().as_ptr() as TupleFormat
    }

    async fn new(&mut self, key_defs: Vec<KeyDef>) -> Result<TupleFormat, BoxError> {
        let key_ptrs: Vec<*const ffi::types::BoxKeyDefT> = key_defs
            .iter()
            .map(|&k| k as *const ffi::types::BoxKeyDefT)
            .collect();

        let ptr = unsafe {
            ffi::tuple::box_tuple_format_new(key_ptrs.as_ptr(), key_ptrs.len() as u16)
        };

        if ptr.is_null() {
            Err(handle_tarantool_error())
        } else {
            Ok(ptr as u64)
        }
    }

    async fn ref_(&mut self, tf: TupleFormat) {
        unsafe { ffi::tuple::box_tuple_format_ref(tf as *mut ffi::types::BoxTupleFormatT) };
    }

    async fn unref(&mut self, tf: TupleFormat) {
        unsafe { ffi::tuple::box_tuple_format_unref(tf as *mut ffi::types::BoxTupleFormatT) };
    }
}
