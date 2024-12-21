use std::os::raw::c_void;
use crate::txffi::{tnt_tx_push, tnt_tx_flush};
pub struct TxCtx {
    pub cb: Box<dyn FnOnce() + Send>,
}
pub fn tx_push(task: Box<TxCtx>) {
    let task_ctx = Box::into_raw(task);
    unsafe {
        tnt_tx_push(tx_task_cb, task_ctx as *mut c_void);
    };
}
pub fn tx_flush() {
    unsafe {
        tnt_tx_flush();
    };
}
#[no_mangle]
extern "C" fn tx_task_cb(args: *mut c_void) {
    let tx_ctx = unsafe { Box::from_raw(args as *mut TxCtx) };
    (tx_ctx.cb)();
}

