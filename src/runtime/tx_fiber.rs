//! A dedicated Tarantool fiber owns the transaction while the guest runs on
//! its worker thread. Host calls are queued here; WASI waits stay off TX.
//!
//! An idle owner waits on a nonblocking socket through the TX event loop,
//! without occupying an eio worker. Dropping the handle disconnects both the
//! job channel and the socket, waking the owner for rollback and exit.
//!
//! This is a cooperative execution model: memtx transactions that access data
//! need MVCC to survive the yields between host calls.

use std::io::{ErrorKind, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{mpsc, Arc};

use anyhow::{anyhow, Result};
use tarantool::{coio::coio_wait, ffi::tarantool::CoIOFlags, fiber, transaction};

use crate::runtime::tarantool_wasm::debug_assert_tx_thread;
use crate::runtime::tx::{tx_push, TxCtx};

pub(crate) type Job = Box<dyn FnOnce() + Send + 'static>;

/// Only thread-safe queue and socket handles cross the TX boundary.
#[derive(Debug)]
pub(crate) struct TxFiber {
    jobs: mpsc::Sender<Job>,
    wake: UnixStream,
}

impl TxFiber {
    /// Called on the guest worker. Wait for TX to confirm creation so a fiber
    /// allocation failure is returned as a begin error, not a dropped-job panic.
    pub(crate) fn spawn() -> Result<Arc<Self>> {
        let (jobs, receiver) = mpsc::channel();
        let (wake, wake_receiver) = UnixStream::pair()?;
        wake.set_nonblocking(true)?;
        wake_receiver.set_nonblocking(true)?;
        let (started_tx, started_rx) = mpsc::channel();
        tx_push(Box::new(TxCtx {
            cb: Box::new(move || {
                let result = fiber::Builder::new()
                    .name("wasm-tx")
                    .func(move || job_loop(receiver, wake_receiver))
                    .start_non_joinable()
                    .map(|_| ())
                    .map_err(|err| err.to_string());
                let _ = started_tx.send(result);
            }),
        }))?;
        started_rx
            .recv()
            .map_err(|_| anyhow!("transaction fiber creation was interrupted"))?
            .map_err(|err| anyhow!(err))?;
        Ok(Arc::new(Self { jobs, wake }))
    }

    /// Submit a job and report owner failure instead of panicking on the
    /// guest worker. Commit and rollback use this to return a WIT error.
    pub(crate) async fn call<F, R>(&self, task: F) -> Result<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        // The job is enqueued before the wake socket is written. The owner may
        // execute it even when that write reports a closed socket, so its reply
        // is authoritative for COMMIT and ROLLBACK.
        let submit_result = self.submit(Box::new(move || {
            let _ = reply_tx.send(task());
        }));
        match reply_rx.await {
            Ok(reply) => Ok(reply),
            Err(_) => Err(submit_result
                .err()
                .unwrap_or_else(|| anyhow!("transaction fiber stopped before replying"))),
        }
    }

    pub(crate) fn submit(&self, job: Job) -> Result<()> {
        self.jobs
            .send(job)
            .map_err(|_| anyhow!("transaction fiber is no longer available"))?;
        // Publish the job before notifying. A full socket already has a pending
        // notification: the owner will recheck the queue after draining it.
        loop {
            match (&self.wake).write(&[1]) {
                Ok(1) => return Ok(()),
                Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                Err(err) if err.kind() == ErrorKind::WouldBlock => return Ok(()),
                Err(err) => return Err(err.into()),
                Ok(_) => return Err(anyhow!("transaction wake socket closed")),
            }
        }
    }
}

fn job_loop(receiver: mpsc::Receiver<Job>, mut wake: UnixStream) {
    debug_assert_tx_thread();
    let mut notifications = [0; 256];
    loop {
        match receiver.try_recv() {
            Ok(job) => {
                // Never unwind through the C fiber trampoline.
                if catch_unwind(AssertUnwindSafe(job)).is_err() {
                    break;
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => break,
            Err(mpsc::TryRecvError::Empty) => match wake.read(&mut notifications) {
                Ok(0) => break,
                Ok(_) => {}
                Err(err) if err.kind() == ErrorKind::Interrupted => {}
                Err(err) if err.kind() == ErrorKind::WouldBlock => {
                    // coio_wait uses libev, unlike coio_call, which would reserve
                    // an eio worker for the entire idle transaction.
                    if coio_wait(wake.as_raw_fd(), CoIOFlags::READ, 365.0 * 86400.0 * 100.0)
                        .is_err()
                    {
                        break;
                    }
                }
                Err(_) => break,
            },
        }
    }
    if transaction::is_in_transaction() {
        let _ = transaction::rollback();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_uses_reply_even_when_wake_fails() {
        let (jobs, receiver) = mpsc::channel::<Job>();
        let (wake, wake_receiver) = UnixStream::pair().unwrap();
        drop(wake_receiver);
        assert!((&wake).write(&[1]).is_err());
        let owner = std::thread::spawn(move || receiver.recv().unwrap()());
        let fiber = TxFiber { jobs, wake };

        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert_eq!(runtime.block_on(fiber.call(|| 42)).unwrap(), 42);
        owner.join().unwrap();
    }

    #[test]
    fn call_reports_closed_owner() {
        let (jobs, receiver) = mpsc::channel::<Job>();
        drop(receiver);
        let (wake, _wake_receiver) = UnixStream::pair().unwrap();
        let fiber = TxFiber { jobs, wake };

        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert!(runtime.block_on(fiber.call(|| 42)).is_err());
    }
}
