use std::thread::{self, JoinHandle};

use anyhow::Result;
use nix::sys::signal::{SigSet, Signal};

pub(super) fn spawn_tt_thread<F, T>(name: String, f: F) -> Result<JoinHandle<T>>
where
    F: FnOnce() -> T,
    F: Send + 'static,
    T: Send + 'static,
{
    let mut newset = SigSet::all();
    newset.remove(Signal::SIGILL);
    newset.remove(Signal::SIGBUS);
    newset.remove(Signal::SIGFPE);
    newset.remove(Signal::SIGSEGV);

    let oldset = SigSet::thread_get_mask()?;

    newset.thread_block()?;
    let handle = thread::Builder::new().name(name).spawn(f);
    let _ = oldset.thread_set_mask();

    Ok(handle?)
}
