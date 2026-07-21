use crate::ffi::tnt::{tnt_tx_flush, tnt_tx_push};
use crate::runtime::tarantool_wasm::{debug_assert_tx_thread, is_tx_thread};
use std::os::raw::c_void;

pub struct TxCtx {
    pub cb: Box<dyn FnOnce() + Send>,
}

/// Queue a callback from a native worker thread for execution on TX.
///
/// `tnt_tx_push` and `tnt_tx_flush` are the only Tarantool APIs in this crate
/// intentionally called off TX: their contract is specifically the bridge
/// from an external thread to TX. The worker must outlive the flush and must
/// be stopped before Tarantool shutdown.
pub fn tx_push(task: Box<TxCtx>) {
    debug_assert!(!is_tx_thread(), "tnt_tx_push must be called outside TX");
    let task_ctx = Box::into_raw(task);
    unsafe {
        tnt_tx_push(tx_task_cb, task_ctx as *mut c_void);
    };
}
pub fn tx_flush() {
    debug_assert!(!is_tx_thread(), "tnt_tx_flush must be called outside TX");
    unsafe {
        tnt_tx_flush();
    };
}

/// Execute non-yielding cleanup on TX without waiting for its result.
///
/// This is used by `Drop` implementations when a guest leaves host resources
/// in a Wasmtime resource table and the table itself is destroyed on a worker.
pub fn run_on_tx_detached<F>(task: F)
where
    F: FnOnce() + Send + 'static,
{
    if is_tx_thread() {
        debug_assert_tx_thread();
        task();
        return;
    }
    tx_push(Box::new(TxCtx { cb: Box::new(task) }));
    tx_flush();
}

#[no_mangle]
extern "C" fn tx_task_cb(args: *mut c_void) {
    debug_assert_tx_thread();
    let tx_ctx = unsafe { Box::from_raw(args as *mut TxCtx) };
    (tx_ctx.cb)();
}
