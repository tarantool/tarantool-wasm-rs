//! High level management of WebAssembly modules executed within Tarantool.
//!
//! This module contains functions exposed to the Lua layer along with runtime
//! helpers required to load, run and interact with WASM components.

mod calls;
mod execution;
mod exports;
mod interpreter;
mod linker;
mod pipe;
#[cfg(feature = "python-ssl-compat")]
mod python_ssl;
mod reentrancy;
mod registry;
pub(crate) mod resources;
mod session;
mod shutdown;
mod store;
mod threading;
mod worker_pool;

use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::{self, OpenOptions},
    net::IpAddr,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc as std_mpsc, Arc, Mutex, RwLock,
    },
    thread::{self, JoinHandle, ThreadId},
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use lazy_static::lazy_static;
use rmpv::Value as RmpvValue;
use tarantool::{coio::coio_call, fiber, trigger::on_shutdown};
use tokio::{runtime::Builder, sync::mpsc};
use wasmtime::{
    component::{
        types::ComponentItem, Component, ComponentExportIndex, HasData, Instance, InstancePre,
        Linker, ResourceTable,
    },
    Cache, CacheConfig, Engine, Precompiled, ResourceLimiter, Store, StoreLimits,
    StoreLimitsBuilder,
};
use wasmtime_wasi::cli::OutputFile;
use wasmtime_wasi::p2::pipe::MemoryInputPipe;
use wasmtime_wasi::sockets::SocketAddrUse;
use wasmtime_wasi::{FsPerms, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpCtxView, WasiHttpView};
use wasmtime_wasi_tls::{WasiTlsCtx, WasiTlsCtxBuilder, WasiTlsCtxView, WasiTlsView};

use crate::ffi;
use crate::runtime::{
    executor::{Executor, TxMode},
    lifecycle, metrics,
    tx::{begin_shutdown, drain_stopping_queue, run_on_tx_detached},
    wasm_fiber::{WasmFiber, WasmFiberCanceler, WasmFiberStatus},
};

use self::{
    calls::{
        execute_call_job, execute_call_job_on_instance, execute_session_call_job_on_instance,
        resolve_call_export_index, CallScratch, ExportCache,
    },
    execution::{
        current_session_id, join_handle_via_coio, worker_result_channel, CallJob, ExportCacheKey,
        WasmRunEntry, WorkerResultSender,
    },
    exports::{collect_exports, export_info_from_component_func},
    interpreter::{
        close_interpreter_impl, run_interpreter_worker,
        tt_wasm_interpreter_call_export_with_ownership, tt_wasm_interpreter_drop_resource,
        tt_wasm_interpreter_drop_resource_detached, InterpreterCommand, SharedInterpreterEntry,
    },
    linker::{build_runtime_linker, runtime_config},
    reentrancy::{current_call_chain, ensure_target_not_active, CallChain},
    registry::{ExportMap, ModuleMode, PreparedComponent, RuntimeState},
    resources::{SessionCallResult, SessionResources},
    session::{close_session_impl, SessionCommand, WasmSession},
    shutdown::shutdown_runtime,
    store::{init_store, reset_store_fuel},
    threading::spawn_tt_thread,
    worker_pool::{StatelessExecution, StatelessWorkerPool},
};

#[allow(unused_imports)]
pub use self::session::{tt_wasm_session_call_function, tt_wasm_session_call_functions};
pub use self::{
    execution::tt_wasm_run_module,
    interpreter::{tt_wasm_interpreter_call, tt_wasm_interpreter_exec, tt_wasm_interpreter_key},
    pipe::{tt_wasm_pipe_spawn, PipeStream, DEFAULT_PIPE_CAPACITY, MAX_PIPE_CAPACITY},
    registry::{
        tt_wasm_drop_module, tt_wasm_export_names, tt_wasm_load_module,
        tt_wasm_precompile_component, tt_wasm_stats, ExportInfo,
    },
    session::{tt_wasm_close_session, tt_wasm_open_session},
    store::WasmConfig,
};
pub(crate) use self::{
    execution::{
        start_wasm_call_function, start_wasm_call_functions, WasmCallHandle, WasmRunHandle,
        WorkerResultReceiver,
    },
    pipe::WasmPipeProcess,
    reentrancy::install_call_chain,
    session::{
        start_wasm_session_call_function_with_ownership,
        start_wasm_session_call_functions_with_ownership, tt_wasm_bind_instance,
        tt_wasm_close_instance_detached, tt_wasm_instance_call_function_with_ownership,
        tt_wasm_instance_drop_resource, tt_wasm_instance_drop_resource_detached,
        WasmInstanceTarget,
    },
    store::SyncWasmStates,
};

lazy_static! {
    static ref runtime_state: RuntimeState = RuntimeState::new();
}

pub(crate) fn is_tx_thread() -> bool {
    thread::current().id() == runtime_state.tx_thread_id
}

/// Assert that an operation requiring Tarantool TX state runs on TX.
///
/// Kept next to the recorded thread identity so runtime and bindings use the
/// same invariant.
#[inline]
pub(crate) fn debug_assert_tx_thread() {
    debug_assert!(
        is_tx_thread(),
        "TX-thread-only Tarantool API called outside the TX thread"
    );
}
