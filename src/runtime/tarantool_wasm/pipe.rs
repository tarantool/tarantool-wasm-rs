use std::{
    collections::{BTreeSet, VecDeque},
    io,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering as AtomicOrdering},
        mpsc as std_mpsc, Arc, Condvar, Mutex,
    },
    task::{Context, Poll, Waker},
    thread::JoinHandle,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};
use bytes::Bytes;
use bytes::BytesMut;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use wasmtime::format_err;
use wasmtime::Store;
use wasmtime_wasi::{
    cli::{IsTerminal, StdinStream, StdoutStream},
    p2::{InputStream, OutputStream, Pollable, StreamError},
};

use super::*;

pub const DEFAULT_PIPE_CAPACITY: usize = 64 * 1024;
pub const MAX_PIPE_CAPACITY: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PipeStream {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug)]
pub(crate) enum PipeTerminalResult {
    Success,
    GuestError(String),
    Canceled,
    RuntimeError(String),
}

impl PipeTerminalResult {
    pub(crate) fn into_result(self) -> Result<()> {
        match self {
            Self::Success => Ok(()),
            Self::GuestError(message) | Self::RuntimeError(message) => Err(anyhow!(message)),
            Self::Canceled => Err(anyhow!("canceled")),
        }
    }
}

pub(crate) enum PipeJoinState {
    Available(JoinHandle<Result<()>>),
    Joining(JoinHandle<Result<()>>),
    Joined(PipeTerminalResult),
}

pub(crate) struct PipeControlBlock {
    pipe_id: u64,
    join: Mutex<Option<PipeJoinState>>,
    join_changed: Condvar,
    cancel_tx: Mutex<Option<WasmFiberCanceler>>,
    endpoints: PipeEndpoints,
    stdin_writes: OperationGate,
    stdout_reads: OperationGate,
    stderr_reads: OperationGate,
    read_any_eof: Mutex<[bool; 2]>,
    terminal: Arc<Mutex<Option<PipeTerminalResult>>>,
}

impl PipeControlBlock {
    fn new(
        pipe_id: u64,
        join_handle: JoinHandle<Result<()>>,
        cancel_tx: WasmFiberCanceler,
        endpoints: PipeEndpoints,
        terminal: Arc<Mutex<Option<PipeTerminalResult>>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            pipe_id,
            join: Mutex::new(Some(PipeJoinState::Available(join_handle))),
            join_changed: Condvar::new(),
            cancel_tx: Mutex::new(Some(cancel_tx)),
            endpoints,
            stdin_writes: OperationGate::default(),
            stdout_reads: OperationGate::default(),
            stderr_reads: OperationGate::default(),
            read_any_eof: Mutex::new([false, false]),
            terminal,
        })
    }

    pub(crate) fn wait_join(self: &Arc<Self>, timeout: Option<Duration>) -> Result<()> {
        let deadline = timeout.map(|duration| Instant::now() + duration);
        let mut state = self.join.lock().unwrap();
        loop {
            match state.as_ref() {
                Some(PipeJoinState::Available(_)) => {
                    let Some(PipeJoinState::Available(handle)) = state.take() else {
                        unreachable!()
                    };
                    *state = Some(PipeJoinState::Joining(handle));
                    continue;
                }
                Some(PipeJoinState::Joining(handle)) if handle.is_finished() => {
                    let Some(PipeJoinState::Joining(handle)) = state.take() else {
                        unreachable!()
                    };
                    let terminal = match handle.join() {
                        Err(_) => {
                            PipeTerminalResult::RuntimeError("pipe worker thread panicked".into())
                        }
                        Ok(_) => self.terminal.lock().unwrap().clone().unwrap_or_else(|| {
                            PipeTerminalResult::RuntimeError(
                                "pipe worker did not publish a terminal result".into(),
                            )
                        }),
                    };
                    *state = Some(PipeJoinState::Joined(terminal.clone()));
                    self.join_changed.notify_all();
                    return terminal.into_result();
                }
                Some(PipeJoinState::Joined(result)) => return result.clone().into_result(),
                Some(PipeJoinState::Joining(_)) => {}
                None => return Err(anyhow!("pipe join state is unavailable")),
            }
            let poll = Duration::from_millis(1);
            let wait = match deadline {
                None => poll,
                Some(deadline) => deadline
                    .checked_duration_since(Instant::now())
                    .ok_or_else(|| anyhow!("pipe join timed out"))?
                    .min(poll),
            };
            if is_tx_thread() {
                drop(state);
                fiber::sleep(wait);
                state = self.join.lock().unwrap();
            } else {
                state = self.join_changed.wait_timeout(state, wait).unwrap().0;
            }
        }
    }

    pub(crate) fn wait_join_for_cleanup(self: &Arc<Self>) -> Result<()> {
        let result = self.wait_join(None);
        match self.joined_snapshot() {
            Some(PipeTerminalResult::RuntimeError(message)) => Err(anyhow!(message)),
            Some(
                PipeTerminalResult::Success
                | PipeTerminalResult::GuestError(_)
                | PipeTerminalResult::Canceled,
            ) => Ok(()),
            None => result,
        }
    }

    pub(crate) fn cancel(&self) {
        // Latch cancellation before closing stdin can wake a waiting guest.
        if self.terminal.lock().unwrap().is_none() {
            if let Some(cancel_tx) = self.cancel_tx.lock().unwrap().as_ref() {
                cancel_tx.request();
            }
        }
        self.endpoints.stdin.close();
    }

    pub(crate) fn close_streams(&self, discard: bool) {
        self.endpoints.stdin.close();
        if discard {
            self.endpoints.stdout.discard_and_close();
            self.endpoints.stderr.discard_and_close();
        }
    }

    pub(crate) fn is_alive(&self) -> bool {
        if self.terminal.lock().unwrap().is_some() || self.endpoints.outputs_closed() {
            return false;
        }
        match self.join.lock().unwrap().as_ref() {
            Some(PipeJoinState::Available(handle) | PipeJoinState::Joining(handle)) => {
                !handle.is_finished()
            }
            Some(PipeJoinState::Joined(_)) | None => false,
        }
    }

    fn joined_snapshot(&self) -> Option<PipeTerminalResult> {
        match self.join.lock().unwrap().as_ref() {
            Some(PipeJoinState::Joined(result)) => Some(result.clone()),
            _ => None,
        }
    }
}

pub(crate) type PipeRegistryEntry = Arc<PipeControlBlock>;

#[derive(Clone)]
pub(crate) struct WasmPipeProcess {
    control: Arc<PipeControlBlock>,
}

impl WasmPipeProcess {
    pub(crate) fn id(&self) -> u64 {
        self.control.pipe_id
    }

    pub(crate) fn write(&self, data: Vec<u8>, timeout: Option<Duration>) -> Result<()> {
        let deadline = deadline(timeout);
        let _guard = self.control.stdin_writes.acquire(deadline)?;
        self.control
            .endpoints
            .stdin
            .write(data, remaining(deadline)?)
    }

    pub(crate) fn read(
        &self,
        stream: PipeStream,
        max_bytes: usize,
        timeout: Option<Duration>,
    ) -> Result<(Vec<u8>, bool)> {
        let deadline = deadline(timeout);
        match stream {
            PipeStream::Stdout => {
                let _guard = self.control.stdout_reads.acquire(deadline)?;
                self.control
                    .endpoints
                    .stdout
                    .read(max_bytes, remaining(deadline)?)
            }
            PipeStream::Stderr => {
                let _guard = self.control.stderr_reads.acquire(deadline)?;
                self.control
                    .endpoints
                    .stderr
                    .read(max_bytes, remaining(deadline)?)
            }
        }
    }

    pub(crate) fn read_any(
        &self,
        max_bytes: usize,
        timeout: Option<Duration>,
    ) -> Result<(Option<PipeStream>, Vec<u8>, bool)> {
        if max_bytes == 0 {
            return Err(anyhow!("max_bytes must be greater than zero"));
        }
        let deadline = deadline(timeout);
        let _stdout = self.control.stdout_reads.acquire(deadline)?;
        let _stderr = self.control.stderr_reads.acquire(deadline)?;
        let mut generation = self.control.endpoints.output_change.generation();
        loop {
            let mut eof = self.control.read_any_eof.lock().unwrap();
            for (index, stream, reader) in [
                (0, PipeStream::Stdout, &self.control.endpoints.stdout),
                (1, PipeStream::Stderr, &self.control.endpoints.stderr),
            ] {
                if eof[index] {
                    continue;
                }
                if let Some((data, at_eof)) = reader.try_read(max_bytes)? {
                    if at_eof {
                        eof[index] = true;
                    }
                    return Ok((Some(stream), data, at_eof));
                }
            }
            if eof[0] && eof[1] {
                return Ok((None, Vec::new(), true));
            }
            drop(eof);
            generation = self
                .control
                .endpoints
                .output_change
                .wait(generation, deadline)?;
        }
    }

    pub(crate) fn close_stdin(&self) {
        self.control.endpoints.stdin.close();
    }

    pub(crate) fn cancel(&self) {
        self.control.cancel();
    }

    pub(crate) fn join(&self, timeout: Option<Duration>) -> Result<()> {
        let result = self.control.wait_join(timeout);
        if self.control.joined_snapshot().is_some() {
            runtime_state.pipes.write().unwrap().remove(&self.id());
        }
        result
    }

    pub(crate) fn close(&self, timeout: Option<Duration>) -> Result<()> {
        self.control.cancel();
        let result = self.control.wait_join(timeout);
        let Some(terminal) = self.control.joined_snapshot() else {
            return result;
        };
        self.control.close_streams(true);
        runtime_state.pipes.write().unwrap().remove(&self.id());
        match terminal {
            PipeTerminalResult::RuntimeError(message) => Err(anyhow!(message)),
            PipeTerminalResult::Success
            | PipeTerminalResult::GuestError(_)
            | PipeTerminalResult::Canceled => Ok(()),
        }
    }

    pub(crate) fn is_alive(&self) -> bool {
        self.control.is_alive()
    }

    pub(crate) fn abandon(&self) {
        self.control.close_streams(true);
        self.control.cancel();
    }
}

fn deadline(timeout: Option<Duration>) -> Option<Instant> {
    timeout.map(|timeout| Instant::now() + timeout)
}

fn remaining(deadline: Option<Instant>) -> Result<Option<Duration>> {
    deadline
        .map(|deadline| {
            deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| anyhow!("pipe operation timed out"))
        })
        .transpose()
}

#[derive(Default)]
struct GateState {
    next: u64,
    serving: u64,
    canceled: BTreeSet<u64>,
}

#[derive(Default)]
struct OperationGate {
    state: Mutex<GateState>,
    changed: Condvar,
}

struct OperationGuard<'a> {
    gate: &'a OperationGate,
}

impl OperationGate {
    fn acquire(&self, deadline: Option<Instant>) -> Result<OperationGuard<'_>> {
        let mut state = self.state.lock().unwrap();
        let ticket = state.next;
        state.next += 1;
        loop {
            if ticket == state.serving {
                return Ok(OperationGuard { gate: self });
            }
            state = match deadline {
                None => self.changed.wait(state).unwrap(),
                Some(deadline) => {
                    let Some(wait) = deadline.checked_duration_since(Instant::now()) else {
                        state.canceled.insert(ticket);
                        return Err(anyhow!("pipe operation timed out"));
                    };
                    let (next_state, timeout) = self.changed.wait_timeout(state, wait).unwrap();
                    state = next_state;
                    if timeout.timed_out() && ticket != state.serving {
                        state.canceled.insert(ticket);
                        return Err(anyhow!("pipe operation timed out"));
                    }
                    state
                }
            };
        }
    }
}

impl Drop for OperationGuard<'_> {
    fn drop(&mut self) {
        let mut state = self.gate.state.lock().unwrap();
        state.serving += 1;
        loop {
            let serving = state.serving;
            if !state.canceled.remove(&serving) {
                break;
            }
            state.serving += 1;
        }
        self.gate.changed.notify_all();
    }
}

fn validate_capacity(capacity: usize) -> Result<()> {
    if !(1..=MAX_PIPE_CAPACITY).contains(&capacity) {
        return Err(anyhow!(
            "pipe capacity must be between 1 and {MAX_PIPE_CAPACITY} bytes"
        ));
    }
    Ok(())
}

pub fn tt_wasm_pipe_spawn(
    module_uid: String,
    config: Option<WasmConfig>,
    capacity: usize,
) -> Result<WasmPipeProcess> {
    lifecycle::ensure_running()?;
    reap_finished_pipes();
    validate_capacity(capacity)?;
    let loaded = runtime_state
        .modules
        .read()
        .unwrap()
        .get(&module_uid)
        .ok_or_else(|| anyhow!("module `{module_uid}` not found"))?
        .clone();
    let mut cfg = loaded.config;
    if let Some(config) = config {
        cfg = config;
    }
    if cfg.max_instructions.is_some() && !loaded.fuel {
        return Err(anyhow!(
            "module `{module_uid}` was loaded without fuel support"
        ));
    }
    let fuel = loaded.fuel;
    let instance_pre = loaded.instance_pre;
    let (streams, endpoints) = bounded_stdio(capacity);
    let terminal = Arc::new(Mutex::new(None));
    let worker_terminal = Arc::clone(&terminal);
    let (canceler_tx, canceler_rx) = std_mpsc::channel();
    let handle = spawn_tt_thread("wasm-pipe".to_string(), move || {
        let engine = if fuel {
            &runtime_state.fuel_engine
        } else {
            &runtime_state.engine
        };
        let wasi_view = match SyncWasmStates::new_with_pipe(cfg.clone(), streams) {
            Ok(state) => state,
            Err(err) => {
                let message = format!("{err:#}");
                let _ = canceler_tx.send(Err(anyhow!(message.clone())));
                return Err(anyhow!(message));
            }
        };
        let mut store = Store::new(engine, wasi_view);
        store.limiter(|state| &mut state.limits);
        if fuel {
            let _ = store.set_fuel(cfg.max_instructions.unwrap_or(u64::MAX));
        }
        crate::runtime::wasm_fiber::configure_epoch_deadline(&mut store, None);
        let fiber = WasmFiber::new(store, instance_pre)?;
        let _ = canceler_tx.send(Ok(fiber.canceler()));
        let result = fiber.block_on_status();
        let mut terminal = worker_terminal.lock().unwrap();
        if terminal.is_none() {
            *terminal = Some(match &result {
                Ok(WasmFiberStatus::Canceled) => PipeTerminalResult::Canceled,
                Ok(WasmFiberStatus::Finished) => PipeTerminalResult::Success,
                Ok(WasmFiberStatus::Suspended) => PipeTerminalResult::RuntimeError(
                    "pipe worker terminated while suspended".into(),
                ),
                Err(err) => PipeTerminalResult::GuestError(format!("{err:#}")),
            });
        }
        result.map(|_| ())
    })?;
    let cancel_tx = match canceler_rx.recv() {
        Ok(Ok(cancel_tx)) => cancel_tx,
        Ok(Err(err)) => {
            let _ = handle.join();
            return Err(err);
        }
        Err(_) => {
            return handle
                .join()
                .unwrap_or_else(|_| Err(anyhow!("pipe worker panicked")))
                .and_then(|_| Err(anyhow!("failed to receive pipe cancel handle")))
        }
    };
    if let Err(err) = lifecycle::ensure_running() {
        cancel_tx.request();
        let _ = handle.join();
        return Err(err);
    }
    let pipe_id = runtime_state.next_run_uid.fetch_add(1, Ordering::Relaxed);
    let control = PipeControlBlock::new(pipe_id, handle, cancel_tx, endpoints, terminal);
    runtime_state
        .pipes
        .write()
        .unwrap()
        .insert(pipe_id, Arc::clone(&control));
    Ok(WasmPipeProcess { control })
}

pub(crate) fn reap_finished_pipes() {
    let entries = runtime_state
        .pipes
        .read()
        .unwrap()
        .iter()
        .map(|(id, entry)| (*id, Arc::clone(entry)))
        .collect::<Vec<_>>();
    let mut reaped = Vec::new();
    for (id, entry) in entries {
        let finished = match entry.join.lock().unwrap().as_ref() {
            Some(PipeJoinState::Joined(_)) => true,
            Some(PipeJoinState::Available(handle) | PipeJoinState::Joining(handle)) => {
                handle.is_finished()
            }
            None => false,
        };
        if finished {
            let _ = entry.wait_join(Some(Duration::ZERO));
            if entry.joined_snapshot().is_some() {
                reaped.push(id);
            }
        }
    }
    if !reaped.is_empty() {
        let mut registry = runtime_state.pipes.write().unwrap();
        for id in reaped {
            registry.remove(&id);
        }
    }
}

fn wake_all(wakers: Vec<Waker>) {
    for waker in wakers {
        waker.wake();
    }
}

fn register_waker(wakers: &mut Vec<Waker>, waker: &Waker) {
    if !wakers.iter().any(|saved| saved.will_wake(waker)) {
        wakers.push(waker.clone());
    }
}

#[derive(Default)]
struct QueueState {
    chunks: VecDeque<Bytes>,
    bytes: usize,
    producer_closed: bool,
    consumer_closed: bool,
    read_wakers: Vec<Waker>,
    write_wakers: Vec<Waker>,
}

struct ByteQueue {
    capacity: usize,
    state: Mutex<QueueState>,
    readable: Condvar,
    writable: Condvar,
    change: Arc<PipeChange>,
}

impl ByteQueue {
    fn new(capacity: usize, change: Arc<PipeChange>) -> Arc<Self> {
        Arc::new(Self {
            capacity,
            state: Mutex::new(QueueState::default()),
            readable: Condvar::new(),
            writable: Condvar::new(),
            change,
        })
    }

    fn deadline(timeout: Option<Duration>) -> Option<Instant> {
        timeout.map(|duration| Instant::now() + duration)
    }

    fn wait<'a>(
        condvar: &Condvar,
        guard: std::sync::MutexGuard<'a, QueueState>,
        deadline: Option<Instant>,
    ) -> Result<std::sync::MutexGuard<'a, QueueState>> {
        match deadline {
            None => Ok(condvar.wait(guard).unwrap()),
            Some(deadline) => {
                let now = Instant::now();
                if now >= deadline {
                    return Err(anyhow!("pipe operation timed out"));
                }
                let (guard, result) = condvar.wait_timeout(guard, deadline - now).unwrap();
                if result.timed_out() {
                    Err(anyhow!("pipe operation timed out"))
                } else {
                    Ok(guard)
                }
            }
        }
    }

    fn write_atomic(&self, bytes: Bytes, timeout: Option<Duration>) -> Result<()> {
        if bytes.len() > self.capacity {
            return Err(anyhow!(
                "pipe write of {} bytes exceeds capacity {}",
                bytes.len(),
                self.capacity
            ));
        }
        if bytes.is_empty() {
            return Ok(());
        }

        let deadline = Self::deadline(timeout);
        let mut state = self.state.lock().unwrap();
        loop {
            if state.consumer_closed {
                return Err(anyhow!("pipe reader is closed"));
            }
            if state.producer_closed {
                return Err(anyhow!("pipe writer is closed"));
            }
            if self.capacity - state.bytes >= bytes.len() {
                state.bytes += bytes.len();
                state.chunks.push_back(bytes);
                let wakers = std::mem::take(&mut state.read_wakers);
                self.readable.notify_all();
                self.change.notify();
                drop(state);
                wake_all(wakers);
                return Ok(());
            }
            state = Self::wait(&self.writable, state, deadline)?;
        }
    }

    fn read_blocking(
        &self,
        max_bytes: usize,
        timeout: Option<Duration>,
    ) -> Result<(Vec<u8>, bool)> {
        if max_bytes == 0 {
            return Err(anyhow!("max_bytes must be greater than zero"));
        }
        let deadline = Self::deadline(timeout);
        let mut state = self.state.lock().unwrap();
        loop {
            if let Some(mut chunk) = state.chunks.pop_front() {
                let size = max_bytes.min(chunk.len());
                let data = chunk.split_to(size);
                if !chunk.is_empty() {
                    state.chunks.push_front(chunk);
                }
                state.bytes -= data.len();
                let wakers = std::mem::take(&mut state.write_wakers);
                self.writable.notify_all();
                drop(state);
                wake_all(wakers);
                return Ok((data.to_vec(), false));
            }
            if state.producer_closed {
                return Ok((Vec::new(), true));
            }
            if state.consumer_closed {
                return Err(anyhow!("pipe reader is closed"));
            }
            state = Self::wait(&self.readable, state, deadline)?;
        }
    }

    fn close_producer(&self) {
        let mut state = self.state.lock().unwrap();
        if state.producer_closed {
            return;
        }
        state.producer_closed = true;
        let read_wakers = std::mem::take(&mut state.read_wakers);
        let write_wakers = std::mem::take(&mut state.write_wakers);
        self.readable.notify_all();
        self.writable.notify_all();
        self.change.notify();
        drop(state);
        wake_all(read_wakers);
        wake_all(write_wakers);
    }

    fn close_consumer(&self, discard: bool) {
        let mut state = self.state.lock().unwrap();
        if state.consumer_closed {
            return;
        }
        state.consumer_closed = true;
        if discard {
            state.chunks.clear();
            state.bytes = 0;
        }
        let read_wakers = std::mem::take(&mut state.read_wakers);
        let write_wakers = std::mem::take(&mut state.write_wakers);
        self.readable.notify_all();
        self.writable.notify_all();
        self.change.notify();
        drop(state);
        wake_all(read_wakers);
        wake_all(write_wakers);
    }

    fn try_read(&self, size: usize) -> std::result::Result<Bytes, StreamError> {
        let mut state = self.state.lock().unwrap();
        if let Some(mut chunk) = state.chunks.pop_front() {
            let data = chunk.split_to(size.min(chunk.len()));
            if !chunk.is_empty() {
                state.chunks.push_front(chunk);
            }
            state.bytes -= data.len();
            let wakers = std::mem::take(&mut state.write_wakers);
            self.writable.notify_all();
            drop(state);
            wake_all(wakers);
            return Ok(data);
        }
        if state.producer_closed || state.consumer_closed {
            Err(StreamError::Closed)
        } else {
            Ok(Bytes::new())
        }
    }

    fn poll_read_ready(&self, cx: &mut Context<'_>) -> Poll<()> {
        let mut state = self.state.lock().unwrap();
        if state.bytes != 0 || state.producer_closed || state.consumer_closed {
            Poll::Ready(())
        } else {
            register_waker(&mut state.read_wakers, cx.waker());
            Poll::Pending
        }
    }

    async fn write_chunk(&self, bytes: Bytes) -> std::result::Result<(), StreamError> {
        if bytes.len() > self.capacity {
            return Err(StreamError::Trap(format_err!(
                "pipe output chunk exceeds capacity"
            )));
        }
        std::future::poll_fn(|cx| {
            let mut state = self.state.lock().unwrap();
            if state.consumer_closed || state.producer_closed {
                return Poll::Ready(Err(StreamError::Closed));
            }
            if self.capacity - state.bytes < bytes.len() {
                register_waker(&mut state.write_wakers, cx.waker());
                return Poll::Pending;
            }
            state.bytes += bytes.len();
            if !bytes.is_empty() {
                state.chunks.push_back(bytes.clone());
            }
            let wakers = std::mem::take(&mut state.read_wakers);
            self.readable.notify_all();
            self.change.notify();
            drop(state);
            wake_all(wakers);
            Poll::Ready(Ok(()))
        })
        .await
    }
}

#[derive(Clone)]
pub(crate) struct PipeWriter {
    queue: Arc<ByteQueue>,
}

impl PipeWriter {
    pub(crate) fn write(&self, data: Vec<u8>, timeout: Option<Duration>) -> Result<()> {
        self.queue.write_atomic(Bytes::from(data), timeout)
    }

    pub(crate) fn close(&self) {
        self.queue.close_producer();
    }
}

#[derive(Clone)]
pub(crate) struct PipeReader {
    queue: Arc<ByteQueue>,
}

impl PipeReader {
    pub(crate) fn read(
        &self,
        max_bytes: usize,
        timeout: Option<Duration>,
    ) -> Result<(Vec<u8>, bool)> {
        self.queue.read_blocking(max_bytes, timeout)
    }

    pub(crate) fn discard_and_close(&self) {
        self.queue.close_consumer(true);
    }

    fn producer_closed(&self) -> bool {
        self.queue.state.lock().unwrap().producer_closed
    }

    fn try_read(&self, max_bytes: usize) -> Result<Option<(Vec<u8>, bool)>> {
        let mut state = self.queue.state.lock().unwrap();
        if let Some(mut chunk) = state.chunks.pop_front() {
            let data = chunk.split_to(max_bytes.min(chunk.len()));
            if !chunk.is_empty() {
                state.chunks.push_front(chunk);
            }
            state.bytes -= data.len();
            let wakers = std::mem::take(&mut state.write_wakers);
            self.queue.writable.notify_all();
            drop(state);
            wake_all(wakers);
            return Ok(Some((data.to_vec(), false)));
        }
        if state.producer_closed {
            Ok(Some((Vec::new(), true)))
        } else if state.consumer_closed {
            Err(anyhow!("pipe reader is closed"))
        } else {
            Ok(None)
        }
    }
}

pub(crate) struct PipeWasiStreams {
    stdin: PipeInputFactory,
    stdout: PipeOutputFactory,
    stderr: PipeOutputFactory,
}

pub(crate) struct PipeEndpoints {
    pub(crate) stdin: PipeWriter,
    pub(crate) stdout: PipeReader,
    pub(crate) stderr: PipeReader,
    output_change: Arc<PipeChange>,
}

impl PipeEndpoints {
    fn outputs_closed(&self) -> bool {
        self.stdout.producer_closed() && self.stderr.producer_closed()
    }
}

pub(crate) fn bounded_stdio(capacity: usize) -> (PipeWasiStreams, PipeEndpoints) {
    let stdin_change = Arc::new(PipeChange::default());
    let output_change = Arc::new(PipeChange::default());
    let stdin = ByteQueue::new(capacity, stdin_change);
    let stdout = ByteQueue::new(capacity, Arc::clone(&output_change));
    let stderr = ByteQueue::new(capacity, Arc::clone(&output_change));
    let stdout_producer = PipeProducer::new(Arc::clone(&stdout));
    let stderr_producer = PipeProducer::new(Arc::clone(&stderr));
    (
        PipeWasiStreams {
            stdin: PipeInputFactory(Arc::clone(&stdin)),
            stdout: PipeOutputFactory(stdout_producer),
            stderr: PipeOutputFactory(stderr_producer),
        },
        PipeEndpoints {
            stdin: PipeWriter { queue: stdin },
            stdout: PipeReader { queue: stdout },
            stderr: PipeReader { queue: stderr },
            output_change,
        },
    )
}

#[derive(Default)]
struct PipeChange {
    generation: Mutex<u64>,
    changed: Condvar,
}

impl PipeChange {
    fn generation(&self) -> u64 {
        *self.generation.lock().unwrap()
    }

    fn notify(&self) {
        let mut generation = self.generation.lock().unwrap();
        *generation = generation.wrapping_add(1);
        self.changed.notify_all();
    }

    fn wait(&self, observed: u64, deadline: Option<Instant>) -> Result<u64> {
        let mut generation = self.generation.lock().unwrap();
        while *generation == observed {
            generation = match deadline {
                None => self.changed.wait(generation).unwrap(),
                Some(deadline) => {
                    let Some(wait) = deadline.checked_duration_since(Instant::now()) else {
                        return Err(anyhow!("pipe operation timed out"));
                    };
                    let (next, timeout) = self.changed.wait_timeout(generation, wait).unwrap();
                    if timeout.timed_out() && *next == observed {
                        return Err(anyhow!("pipe operation timed out"));
                    }
                    next
                }
            };
        }
        Ok(*generation)
    }
}

impl PipeWasiStreams {
    pub(crate) fn install(self, builder: &mut wasmtime_wasi::WasiCtxBuilder) {
        builder
            .stdin(self.stdin)
            .stdout(self.stdout)
            .stderr(self.stderr);
    }
}

#[derive(Clone)]
struct PipeInputFactory(Arc<ByteQueue>);

impl IsTerminal for PipeInputFactory {
    fn is_terminal(&self) -> bool {
        false
    }
}

impl StdinStream for PipeInputFactory {
    fn async_stream(&self) -> Box<dyn AsyncRead + Send + Sync> {
        Box::new(PipeAsyncReader(Arc::clone(&self.0)))
    }

    fn p2_stream(&self) -> Box<dyn InputStream> {
        Box::new(PipeInputStream(Arc::clone(&self.0)))
    }
}

struct PipeProducer {
    queue: Arc<ByteQueue>,
    owners: Arc<AtomicUsize>,
}

impl PipeProducer {
    fn new(queue: Arc<ByteQueue>) -> Self {
        Self {
            queue,
            owners: Arc::new(AtomicUsize::new(1)),
        }
    }
}

impl Clone for PipeProducer {
    fn clone(&self) -> Self {
        self.owners.fetch_add(1, AtomicOrdering::Relaxed);
        Self {
            queue: Arc::clone(&self.queue),
            owners: Arc::clone(&self.owners),
        }
    }
}

impl Drop for PipeProducer {
    fn drop(&mut self) {
        if self.owners.fetch_sub(1, AtomicOrdering::AcqRel) == 1 {
            self.queue.close_producer();
        }
    }
}

struct PipeOutputFactory(PipeProducer);

impl IsTerminal for PipeOutputFactory {
    fn is_terminal(&self) -> bool {
        false
    }
}

impl StdoutStream for PipeOutputFactory {
    fn async_stream(&self) -> Box<dyn AsyncWrite + Send + Sync> {
        Box::new(PipeAsyncWriter(self.0.clone()))
    }

    fn p2_stream(&self) -> Box<dyn OutputStream> {
        Box::new(PipeOutputStream {
            producer: self.0.clone(),
            staging: BytesMut::new(),
            staging_capacity: self.0.queue.capacity.min(8192),
            flush_pending: false,
            pending: None,
            pending_error: None,
        })
    }
}

struct PipeInputStream(Arc<ByteQueue>);

#[async_trait::async_trait]
impl InputStream for PipeInputStream {
    fn read(&mut self, size: usize) -> std::result::Result<Bytes, StreamError> {
        self.0.try_read(size)
    }
}

#[async_trait::async_trait]
impl Pollable for PipeInputStream {
    async fn ready(&mut self) {
        std::future::poll_fn(|cx| self.0.poll_read_ready(cx)).await
    }
}

struct PipeOutputStream {
    producer: PipeProducer,
    staging: BytesMut,
    staging_capacity: usize,
    flush_pending: bool,
    pending: Option<tokio::task::JoinHandle<std::result::Result<(), StreamError>>>,
    pending_error: Option<StreamError>,
}

#[async_trait::async_trait]
impl OutputStream for PipeOutputStream {
    fn write(&mut self, bytes: Bytes) -> std::result::Result<(), StreamError> {
        if let Some(err) = self.pending_error.take() {
            return Err(err);
        }
        if self.pending.is_some() || self.flush_pending {
            return Err(StreamError::Trap(format_err!(
                "pipe output write called before ready"
            )));
        }
        let available = self.staging_capacity - self.staging.len();
        if bytes.len() > available {
            return Err(StreamError::Trap(format_err!(
                "write of {} bytes exceeds pipe staging budget {}",
                bytes.len(),
                available
            )));
        }
        self.staging.extend_from_slice(&bytes);
        let chunk = self.staging.split().freeze();
        let producer = self.producer.clone();
        self.pending = Some(tokio::spawn(async move {
            producer.queue.write_chunk(chunk).await
        }));
        Ok(())
    }

    fn flush(&mut self) -> std::result::Result<(), StreamError> {
        if let Some(err) = self.pending_error.take() {
            return Err(err);
        }
        self.flush_pending = true;
        Ok(())
    }

    fn check_write(&mut self) -> std::result::Result<usize, StreamError> {
        if let Some(err) = self.pending_error.take() {
            return Err(err);
        }
        let state = self.producer.queue.state.lock().unwrap();
        if state.consumer_closed || state.producer_closed {
            return Err(StreamError::Closed);
        }
        if self.flush_pending || self.pending.is_some() {
            return Ok(0);
        }
        Ok(self.staging_capacity - self.staging.len())
    }
}

#[async_trait::async_trait]
impl Pollable for PipeOutputStream {
    async fn ready(&mut self) {
        if let Some(pending) = self.pending.take() {
            match pending.await {
                Ok(Ok(())) => {}
                Ok(Err(err)) => self.pending_error = Some(err),
                Err(err) => {
                    self.pending_error = Some(StreamError::Trap(format_err!(
                        "pipe output drain task failed: {err}"
                    )))
                }
            }
        }
        self.flush_pending = false;
    }
}

struct PipeAsyncReader(Arc<ByteQueue>);

impl AsyncRead for PipeAsyncReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.0.try_read(buf.remaining()) {
            Ok(bytes) if bytes.is_empty() => {
                if self.0.poll_read_ready(cx).is_ready() {
                    cx.waker().wake_by_ref();
                }
                Poll::Pending
            }
            Ok(bytes) => {
                buf.put_slice(&bytes);
                Poll::Ready(Ok(()))
            }
            Err(StreamError::Closed) => Poll::Ready(Ok(())),
            Err(err) => Poll::Ready(Err(io::Error::other(err.to_string()))),
        }
    }
}

struct PipeAsyncWriter(PipeProducer);

impl AsyncWrite for PipeAsyncWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut state = self.0.queue.state.lock().unwrap();
        if state.consumer_closed || state.producer_closed {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "pipe is closed",
            )));
        }
        let available = self.0.queue.capacity - state.bytes;
        if available == 0 {
            register_waker(&mut state.write_wakers, cx.waker());
            return Poll::Pending;
        }
        let size = available.min(buf.len());
        state.bytes += size;
        state.chunks.push_back(Bytes::copy_from_slice(&buf[..size]));
        let wakers = std::mem::take(&mut state.read_wakers);
        self.0.queue.readable.notify_all();
        self.0.queue.change.notify();
        drop(state);
        wake_all(wakers);
        Poll::Ready(Ok(size))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_async(test: impl std::future::Future<Output = ()>) {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(test);
    }

    #[test]
    fn cloned_p2_output_handles_have_independent_staging_budgets() {
        run_async(async {
            let (streams, endpoints) = bounded_stdio(8);
            let PipeWasiStreams {
                stdin,
                stdout,
                stderr,
            } = streams;
            let mut a = stdout.p2_stream();
            let mut b = stdout.p2_stream();

            // Neither check_write call reserves shared queue capacity. In
            // particular, A must not prevent B from accepting its local chunk.
            assert_eq!(a.check_write().unwrap(), 8);
            assert_eq!(b.check_write().unwrap(), 8);
            b.write(Bytes::from_static(b"BBBB")).unwrap();
            a.write(Bytes::from_static(b"AAAA")).unwrap();
            b.flush().unwrap();
            a.flush().unwrap();
            b.ready().await;
            a.ready().await;

            drop(a);
            drop(b);
            drop(stdout);
            drop(stdin);
            drop(stderr);

            let (first, first_eof) = endpoints.stdout.read(8, None).unwrap();
            let (second, second_eof) = endpoints.stdout.read(8, None).unwrap();
            let (tail, tail_eof) = endpoints.stdout.read(8, None).unwrap();
            assert!(!first_eof);
            assert!(!second_eof);
            assert!(tail_eof);
            assert!(tail.is_empty());

            // Scheduling between distinct handles is intentionally unordered,
            // but each write must remain an intact queue chunk.
            let mut chunks = vec![first, second];
            chunks.sort();
            assert_eq!(chunks, vec![b"AAAA".to_vec(), b"BBBB".to_vec()]);
        });
    }

    #[test]
    fn cloned_p2_input_handles_share_cursor_readiness_and_eof() {
        run_async(async {
            let (streams, endpoints) = bounded_stdio(8);
            let mut a = streams.stdin.p2_stream();
            let mut b = streams.stdin.p2_stream();

            endpoints.stdin.write(b"abcdef".to_vec(), None).unwrap();
            a.ready().await;
            b.ready().await;
            assert_eq!(a.read(2).unwrap(), Bytes::from_static(b"ab"));
            assert_eq!(b.read(2).unwrap(), Bytes::from_static(b"cd"));

            endpoints.stdin.close();
            a.ready().await;
            assert_eq!(a.read(8).unwrap(), Bytes::from_static(b"ef"));
            b.ready().await;
            assert!(matches!(b.read(1), Err(StreamError::Closed)));
            assert!(matches!(a.read(1), Err(StreamError::Closed)));
        });
    }

    #[test]
    fn host_atomic_write_timeout_accepts_no_bytes() {
        let (_streams, endpoints) = bounded_stdio(4);
        endpoints.stdin.write(b"full".to_vec(), None).unwrap();

        let error = endpoints
            .stdin
            .write(b"x".to_vec(), Some(Duration::from_millis(5)))
            .unwrap_err();
        assert!(error.to_string().contains("timed out"));

        let (data, eof) = endpoints.stdin.queue.read_blocking(4, None).unwrap();
        assert_eq!(data, b"full");
        assert!(!eof);
        endpoints.stdin.close();
        let (tail, eof) = endpoints.stdin.queue.read_blocking(4, None).unwrap();
        assert!(tail.is_empty());
        assert!(eof);
    }

    #[test]
    fn output_queue_reports_eof_only_after_buffer_is_drained() {
        run_async(async {
            let (streams, endpoints) = bounded_stdio(8);
            let PipeWasiStreams {
                stdin,
                stdout,
                stderr,
            } = streams;
            let mut output = stdout.p2_stream();
            output.write(Bytes::from_static(b"tail")).unwrap();
            output.ready().await;

            drop(output);
            drop(stdout);
            drop(stdin);
            drop(stderr);

            let (data, eof) = endpoints.stdout.read(8, None).unwrap();
            assert_eq!(data, b"tail");
            assert!(!eof);
            let (data, eof) = endpoints.stdout.read(8, None).unwrap();
            assert!(data.is_empty());
            assert!(eof);
        });
    }

    #[test]
    fn join_timeout_leaves_joining_state_retryable_and_caches_result() {
        let (_streams, endpoints) = bounded_stdio(8);
        let terminal = Arc::new(Mutex::new(None));
        let worker_terminal = Arc::clone(&terminal);
        let worker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            *worker_terminal.lock().unwrap() =
                Some(PipeTerminalResult::GuestError("guest failed".into()));
            Ok(())
        });
        let (cancel_sender, _cancel_rx) = tokio::sync::mpsc::channel(1);
        let cancel_tx = WasmFiberCanceler::for_test(cancel_sender);
        let control = PipeControlBlock::new(1, worker, cancel_tx, endpoints, terminal);

        let timeout = control
            .wait_join(Some(Duration::from_millis(1)))
            .unwrap_err();
        assert!(timeout.to_string().contains("timed out"));
        assert!(matches!(
            control.join.lock().unwrap().as_ref(),
            Some(PipeJoinState::Joining(_))
        ));

        let first = control
            .wait_join(Some(Duration::from_secs(1)))
            .unwrap_err()
            .to_string();
        let second = control
            .wait_join(Some(Duration::ZERO))
            .unwrap_err()
            .to_string();
        assert_eq!(first, "guest failed");
        assert_eq!(second, first);
        assert!(matches!(
            control.join.lock().unwrap().as_ref(),
            Some(PipeJoinState::Joined(PipeTerminalResult::GuestError(message)))
                if message == "guest failed"
        ));
    }
}
