use crate::bindings::error::handle_tarantool_error;
use crate::bindings::tarantool::tarantool::tuple_format;
use crate::bindings::tarantool::tarantool::types::{BoxError, KeyDef, TupleFormat};
use crate::ffi;
use crate::runtime::tarantool_wasm::SyncWasmStates;

use tarantool::tuple::TupleFormat as TarantoolTupleFormat;

impl tuple_format::Host for SyncWasmStates {
    async fn default(&mut self) -> TupleFormat {
        TupleFormat {
            ptr: TarantoolTupleFormat::default().as_ptr() as u64,
        }
    }

    async fn new(&mut self, key_defs: Vec<KeyDef>) -> Result<TupleFormat, BoxError> {
        let key_ptrs: Vec<*const ffi::types::BoxKeyDefT> = key_defs
            .iter()
            .map(|&k| k.ptr as *const ffi::types::BoxKeyDefT)
            .collect();

        let ptr =
            unsafe { ffi::tuple::box_tuple_format_new(key_ptrs.as_ptr(), key_ptrs.len() as u16) };

        if ptr.is_null() {
            Err(handle_tarantool_error())
        } else {
            Ok(TupleFormat { ptr: ptr as u64 })
        }
    }

    async fn ref_(&mut self, tf: TupleFormat) {
        unsafe { ffi::tuple::box_tuple_format_ref(tf.ptr as *mut ffi::types::BoxTupleFormatT) };
    }

    async fn unref(&mut self, tf: TupleFormat) {
        unsafe { ffi::tuple::box_tuple_format_unref(tf.ptr as *mut ffi::types::BoxTupleFormatT) };
    }
}
