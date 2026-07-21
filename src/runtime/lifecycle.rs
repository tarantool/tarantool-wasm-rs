//! Process-wide lifecycle coordination for native WASM workers.
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

use anyhow::{anyhow, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum RuntimeLifecycle {
    Running = 0,
    Stopping = 1,
    Stopped = 2,
}

static LIFECYCLE: AtomicU8 = AtomicU8::new(RuntimeLifecycle::Running as u8);
static ACTIVE_WORKERS: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn state() -> RuntimeLifecycle {
    match LIFECYCLE.load(Ordering::Acquire) {
        0 => RuntimeLifecycle::Running,
        1 => RuntimeLifecycle::Stopping,
        2 => RuntimeLifecycle::Stopped,
        value => panic!("invalid runtime lifecycle state: {value}"),
    }
}

pub(crate) fn ensure_running() -> Result<()> {
    match state() {
        RuntimeLifecycle::Running => Ok(()),
        RuntimeLifecycle::Stopping => Err(anyhow!("WASM runtime is stopping")),
        RuntimeLifecycle::Stopped => Err(anyhow!("WASM runtime is stopped")),
    }
}

pub(crate) fn begin_stopping() -> bool {
    LIFECYCLE
        .compare_exchange(
            RuntimeLifecycle::Running as u8,
            RuntimeLifecycle::Stopping as u8,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_ok()
}

pub(crate) fn finish_stopping() {
    LIFECYCLE.store(RuntimeLifecycle::Stopped as u8, Ordering::Release);
}

pub(crate) fn active_workers() -> usize {
    ACTIVE_WORKERS.load(Ordering::Acquire)
}

pub(crate) struct WorkerGuard;

impl WorkerGuard {
    pub(crate) fn acquire() -> Result<Self> {
        ensure_running()?;
        ACTIVE_WORKERS.fetch_add(1, Ordering::AcqRel);
        if let Err(err) = ensure_running() {
            ACTIVE_WORKERS.fetch_sub(1, Ordering::AcqRel);
            return Err(err);
        }
        Ok(Self)
    }
}

impl Drop for WorkerGuard {
    fn drop(&mut self) {
        ACTIVE_WORKERS.fetch_sub(1, Ordering::AcqRel);
    }
}
