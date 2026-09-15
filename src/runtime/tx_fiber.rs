//! Dedicated Tarantool fiber that owns a single `box` transaction.
//!
//! A Tarantool transaction is attached to the calling fiber and only one
//! transaction may be active per fiber (`tarantool::transaction`). The Wasmtime
//! guest, however, must be free to block on WASI I/O (e.g. wait for the next
//! client message on a `wasm.pipe`) between the statements of one transaction.
//! Driving the guest *on the TX thread* makes such a wait park the whole TX
//! scheduler, which deadlocks the instance.
//!
//! Instead, the guest always runs on its own OS thread and every host call that
//! must touch TX during a transaction is submitted as a [`Job`] to one
//! long-lived fiber created at `transaction.begin`. Because all jobs for a
//! transaction run on the same fiber, the fiber-local `box` transaction is
//! preserved, while the guest's own I/O waits stay harmless.
//!
//! Between jobs the fiber waits for the next one with a `coio_call`-based
//! cooperative receive (the same idiom as
//! [`crate::runtime::tarantool_wasm::execution::WorkerResultReceiver::recv`]),
//! so it yields instead of parking the TX thread. Dropping the [`TxFiber`]
//! handle closes the job channel, which lets the loop wake up, roll back any
//! transaction the guest failed to close, and exit.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{mpsc as std_mpsc, Arc, Mutex};

use anyhow::Result;
use tarantool::{coio::coio_call, fiber, transaction as tnt_transaction};

use crate::runtime::tx::{tx_push, TxCtx};

/// A unit of work that must run on the transaction's Tarantool fiber.
///
/// Jobs do not return a value; the caller arranges for the result to travel
/// back through a channel it owns.
pub(crate) type Job = Box<dyn FnOnce() + Send + 'static>;

/// Handle to the dedicated fiber owning one Tarantool transaction.
#[derive(Debug)]
pub(crate) struct TxFiber {
    jobs: std_mpsc::Sender<Job>,
}

impl TxFiber {
    /// Spawn a fresh transaction fiber.
    ///
    /// Must be called from outside the TX thread (i.e. from the guest's OS
    /// thread), like any other [`tx_push`].
    pub(crate) fn spawn() -> Result<Arc<Self>> {
        let (jobs, receiver) = std_mpsc::channel::<Job>();
        let receiver = Arc::new(Mutex::new(receiver));
        tx_push(Box::new(TxCtx {
            cb: Box::new(move || {
                // `start_non_joinable` transfers execution to the new fiber
                // immediately and returns once the fiber first yields (at its
                // first `coio_call`), so this callback stays short.
                let _ = fiber::Builder::new()
                    .name("wasm-tx")
                    .func(move || job_loop(receiver))
                    .start_non_joinable();
            }),
        }))?;
        Ok(Arc::new(Self { jobs }))
    }

    /// Queue a job for the transaction fiber.
    ///
    /// Fails only if the fiber has already exited (which closes the channel).
    pub(crate) fn submit(&self, job: Job) -> std::result::Result<(), Job> {
        self.jobs.send(job).map_err(|err| err.0)
    }
}

/// The transaction fiber body: wait for jobs, run them on TX, repeat.
fn job_loop(receiver: Arc<Mutex<std_mpsc::Receiver<Job>>>) {
    loop {
        let mut job: Option<Job> = None;
        {
            let receiver = Arc::clone(&receiver);
            // The `Mutex` guard is held across the blocking `recv`, but the
            // producer side only owns the `Sender`, so there is no contention;
            // the guard exists solely to make the `Receiver` shareable with the
            // eio worker in a sound way. `coio_call` yields the fiber, and the
            // eio task returns once a job is sent or the channel is closed.
            coio_call(
                &mut |receiver: Box<Arc<Mutex<std_mpsc::Receiver<Job>>>>| {
                    let guard = receiver.lock().unwrap();
                    job = guard.recv().ok();
                    0
                },
                receiver,
            );
        }

        match job {
            Some(job) => {
                // A panic in a host call would otherwise unwind across the
                // fiber's C trampoline and abort the process. Catch it, leave
                // the transaction to the unconditional rollback below, and let
                // the caller observe its dropped reply channel as a failure.
                if catch_unwind(AssertUnwindSafe(job)).is_err() {
                    break;
                }
            }
            // All senders dropped: the transaction is over (or the guest is
            // gone). Fall through to the rollback.
            None => break,
        }
    }

    if tnt_transaction::is_in_transaction() {
        let _ = tnt_transaction::rollback();
    }
}
