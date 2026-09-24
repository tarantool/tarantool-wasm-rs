//! Fallible wrapper over Tarantool's index iterator C API.
//!
//! `tarantool::index::IndexIterator::next` returns `None` both at EOF and
//! when `box_iterator_next` fails, so WIT callers need this wrapper.

use std::ptr::null_mut;

use crate::bindings::error::handle_tarantool_error;
use crate::bindings::helpers::debug_assert_tx_thread;
use crate::bindings::tarantool::tarantool::types::{BoxError, Index};
use crate::runtime::tarantool_wasm::is_tx_thread;

use tarantool::ffi::tarantool as tnt_ffi;
use tarantool::index::IteratorType;
use tarantool::tuple::Tuple;

pub struct FallibleIndexIterator {
    ptr: *mut tnt_ffi::BoxIterator,
    // The native iterator can retain pointers into the encoded key.
    _key: Vec<u8>,
}

// Only an opaque handle crosses threads. Construction, next, and destruction
// are always dispatched to the owning TX thread by the callers.
unsafe impl Send for FallibleIndexIterator {}

impl FallibleIndexIterator {
    pub fn new(index: Index, mode: IteratorType, key: Vec<u8>) -> Result<Self, BoxError> {
        debug_assert_tx_thread();
        let range = key.as_ptr_range();
        let ptr = unsafe {
            tnt_ffi::box_index_iterator(
                index.space_id,
                index.id,
                mode as i32,
                range.start.cast(),
                range.end.cast(),
            )
        };
        if ptr.is_null() {
            return Err(handle_tarantool_error());
        }
        Ok(Self { ptr, _key: key })
    }

    pub fn next(&mut self) -> Result<Option<Tuple>, BoxError> {
        debug_assert_tx_thread();
        let mut result = null_mut();
        let rc = unsafe { tnt_ffi::box_iterator_next(self.ptr, &mut result) };
        if rc != 0 {
            return Err(handle_tarantool_error());
        }
        Ok(Tuple::try_from_ptr(result))
    }
}

impl Drop for FallibleIndexIterator {
    fn drop(&mut self) {
        if !is_tx_thread() {
            // TX shutdown may reject a deferred destructor. Freeing a native
            // iterator from the guest worker is unsafe; retain its key too.
            std::mem::forget(std::mem::take(&mut self._key));
            return;
        }
        unsafe { tnt_ffi::box_iterator_free(self.ptr) };
    }
}
