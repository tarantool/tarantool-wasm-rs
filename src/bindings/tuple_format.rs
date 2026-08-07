use crate::bindings::error::handle_tarantool_error;
use crate::bindings::helpers::{debug_assert_tx_thread, illegal_params};
use crate::bindings::tarantool::tarantool::tuple_format;
use crate::bindings::tarantool::tarantool::types::{BoxError, KeyDef, TupleFormat};
use crate::ffi;
use crate::runtime::tarantool_wasm::SyncWasmStates;

use tarantool::tuple::TupleFormat as TarantoolTupleFormat;

impl tuple_format::Host for SyncWasmStates {
    async fn default(&mut self) -> TupleFormat {
        let ptr = self
            .executor()
            .execute(|| {
                debug_assert_tx_thread();
                TarantoolTupleFormat::default().as_ptr() as u64
            })
            .await;
        self.register_tuple_format(ptr, false)
            .expect("failed to register default tuple-format")
    }

    async fn new(&mut self, key_defs: Vec<KeyDef>) -> Result<TupleFormat, BoxError> {
        let key_count = u16::try_from(key_defs.len())
            .map_err(|_| illegal_params("too many key definitions for tuple format"))?;
        let key_ptrs = key_defs
            .into_iter()
            .map(|key_def| self.key_def_ptr(key_def))
            .collect::<Result<Vec<u64>, _>>()?;
        let ptr = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                let key_ptrs = key_ptrs
                    .iter()
                    .map(|ptr| *ptr as *const ffi::types::BoxKeyDefT)
                    .collect::<Vec<_>>();
                let ptr = unsafe { ffi::tuple::box_tuple_format_new(key_ptrs.as_ptr(), key_count) };
                if ptr.is_null() {
                    Err(handle_tarantool_error())
                } else {
                    Ok(ptr as u64)
                }
            })
            .await?;
        self.register_tuple_format(ptr, true)
    }

    async fn retain(&mut self, tf: TupleFormat) -> wasmtime::Result<()> {
        let (ptr, native_owned) = self
            .retain_tuple_format_handle(tf)
            .map_err(|err| wasmtime::Error::msg(err.message))?;
        if native_owned {
            self.executor()
                .execute(move || {
                    debug_assert_tx_thread();
                    unsafe {
                        ffi::tuple::box_tuple_format_ref(ptr as *mut ffi::types::BoxTupleFormatT)
                    };
                })
                .await;
        }
        Ok(())
    }

    async fn release(&mut self, tf: TupleFormat) -> wasmtime::Result<()> {
        let (ptr, native_owned) = self
            .release_tuple_format_handle(tf)
            .map_err(|err| wasmtime::Error::msg(err.message))?;
        if native_owned {
            self.executor()
                .execute(move || {
                    debug_assert_tx_thread();
                    unsafe {
                        ffi::tuple::box_tuple_format_unref(ptr as *mut ffi::types::BoxTupleFormatT)
                    };
                })
                .await;
        }
        Ok(())
    }
}
