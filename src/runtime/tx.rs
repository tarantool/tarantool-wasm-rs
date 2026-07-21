use crate::ffi::tnt::{tnt_tx_flush, tnt_tx_push};
use crate::runtime::lifecycle::{self, RuntimeLifecycle};
use crate::runtime::tarantool_wasm::{debug_assert_tx_thread, is_tx_thread};
use anyhow::{anyhow, Result};
use std::collections::VecDeque;
use std::os::raw::c_void;
use std::sync::Mutex;

static TX_SUBMIT_LOCK: Mutex<()> = Mutex::new(());
static STOPPING_TX_QUEUE: Mutex<VecDeque<Box<TxCtx>>> = Mutex::new(VecDeque::new());

pub struct TxCtx {
    pub cb: Box<dyn FnOnce() + Send>,
}

/// Queue a callback from a native worker thread for execution on TX.
///
/// `tnt_tx_push` and `tnt_tx_flush` are the only Tarantool APIs in this crate
/// intentionally called off TX: their contract is specifically the bridge
/// from an external thread to TX. The worker must outlive the flush and must
/// be stopped before Tarantool shutdown.
pub fn tx_push(task: Box<TxCtx>) -> Result<()> {
    debug_assert!(!is_tx_thread(), "tnt_tx_push must be called outside TX");
    let _submit_guard = TX_SUBMIT_LOCK.lock().unwrap();
    match lifecycle::state() {
        RuntimeLifecycle::Running => {
            let task_ctx = Box::into_raw(task);
            unsafe {
                tnt_tx_push(tx_task_cb, task_ctx as *mut c_void);
                tnt_tx_flush();
            };
            Ok(())
        }
        RuntimeLifecycle::Stopping => {
            STOPPING_TX_QUEUE.lock().unwrap().push_back(task);
            Ok(())
        }
        RuntimeLifecycle::Stopped => Err(anyhow!("WASM runtime is stopped")),
    }
}

pub(crate) fn begin_shutdown() -> bool {
    debug_assert_tx_thread();
    let _submit_guard = TX_SUBMIT_LOCK.lock().unwrap();
    lifecycle::begin_stopping()
}

pub(crate) fn drain_stopping_queue() {
    debug_assert_tx_thread();
    loop {
        let task = STOPPING_TX_QUEUE.lock().unwrap().pop_front();
        let Some(task) = task else {
            break;
        };
        (task.cb)();
    }
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
    let _ = tx_push(Box::new(TxCtx { cb: Box::new(task) }));
}

#[no_mangle]
extern "C" fn tx_task_cb(args: *mut c_void) {
    debug_assert_tx_thread();
    let tx_ctx = unsafe { Box::from_raw(args as *mut TxCtx) };
    (tx_ctx.cb)();
}
