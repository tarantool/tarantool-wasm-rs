use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::bindings::docs::adder::experimental_read;
use crate::bindings::helpers::{debug_assert_tx_thread, unknown_error, validate_array};
use crate::bindings::tarantool::tarantool::types::{
    BoxError, BoxTuple, Index, IteratorType, Space,
};
use crate::bindings::types::convert_iterator_type;
use crate::runtime::tarantool_wasm::SyncWasmStates;
use crate::runtime::tx::run_on_tx_detached;
use crate::runtime::tx_fiber::TxFiber;

use mlua::{Function, Lua, ObjectLike, Table};
use tarantool::tlua::AsLua;
use tarantool::tuple::Tuple;
use wasmtime::component::Resource;

static NEXT_READ_ID: AtomicU64 = AtomicU64::new(1);

fn view_key(id: u64) -> String {
    format!("tarantool-wasm-rs.read-view.{id}")
}

fn iterator_key(id: u64) -> String {
    format!("tarantool-wasm-rs.read-iterator.{id}")
}

fn with_tx_lua<R>(f: impl FnOnce(&Lua) -> mlua::Result<R>) -> Result<R, BoxError> {
    debug_assert_tx_thread();
    let lua_thread = tarantool::lua_state();
    // The current job runs on TX and owns this Lua coroutine for the call.
    let state = lua_thread.as_lua().cast::<mlua::ffi::lua_State>();
    let lua = unsafe { Lua::get_or_init_from_ptr(state) };
    f(lua).map_err(|err| {
        let message = err.to_string();
        if let Some((_, reason)) = message.split_once("snapshot unavailable:") {
            let reason = reason.lines().next().unwrap_or_default();
            unknown_error(format!("snapshot unavailable:{reason}"))
        } else {
            unknown_error(format!("snapshot read failed: {message}"))
        }
    })
}

fn with_read_lua<R>(f: impl FnOnce(&Lua, &Table) -> mlua::Result<R>) -> Result<R, BoxError> {
    with_tx_lua(|lua| {
        let bridge: Table = lua.load(include_str!("read_bridge.lua")).eval()?;
        f(lua, &bridge)
    })
}

fn view(lua: &Lua, id: u64) -> mlua::Result<Table> {
    lua.named_registry_value(&view_key(id))
}

fn cleanup_iterator(id: u64) {
    let _ = with_tx_lua(|lua| lua.unset_named_registry_value(&iterator_key(id)));
}

fn cleanup_view(id: u64, iterators: Vec<u64>) -> Result<(), BoxError> {
    with_tx_lua(|lua| {
        for iterator in iterators {
            let _ = lua.unset_named_registry_value(&iterator_key(iterator));
        }
        let snapshot: Option<Table> = lua.named_registry_value(&view_key(id))?;
        if let Some(snapshot) = snapshot {
            snapshot.call_method::<()>("close", ())?;
        }
        lua.unset_named_registry_value(&view_key(id))
    })
}

struct PendingOpen {
    id: u64,
    canceled: Arc<AtomicBool>,
    armed: bool,
}

impl Drop for PendingOpen {
    fn drop(&mut self) {
        if self.armed {
            self.canceled.store(true, Ordering::Release);
            let id = self.id;
            run_on_tx_detached(move || {
                let _ = cleanup_view(id, Vec::new());
            });
        }
    }
}

struct PendingCleanup {
    id: u64,
    iterators: Option<Vec<u64>>,
}

impl Drop for PendingCleanup {
    fn drop(&mut self) {
        if let Some(iterators) = self.iterators.take() {
            let id = self.id;
            run_on_tx_detached(move || {
                let _ = cleanup_view(id, iterators);
            });
        }
    }
}

fn tuple_ptr(tuple: Tuple) -> u64 {
    let ptr = tuple.as_ptr() as u64;
    std::mem::forget(tuple);
    ptr
}

struct ReadContextState {
    id: u64,
    fiber: Mutex<Option<Arc<TxFiber>>>,
    iterators: Mutex<Vec<u64>>,
}

impl ReadContextState {
    fn fiber(&self) -> Result<Arc<TxFiber>, BoxError> {
        self.fiber
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| unknown_error("read context is closed"))
    }

    fn detach(&self) -> Option<(Arc<TxFiber>, Vec<u64>)> {
        let fiber = self.fiber.lock().unwrap().take()?;
        Some((fiber, self.iterators.lock().unwrap().drain(..).collect()))
    }
}

async fn close_context(context: &ReadContextState) -> Result<(), BoxError> {
    let Some((fiber, iterators)) = context.detach() else {
        return Ok(());
    };
    let id = context.id;
    let mut pending = PendingCleanup {
        id,
        iterators: Some(iterators.clone()),
    };
    let result = call_on_read_fiber(fiber, move || cleanup_view(id, iterators)).await;
    if result.is_ok() {
        pending.iterators = None;
    }
    result
}

async fn call_on_read_fiber<T: Send + 'static>(
    fiber: Arc<TxFiber>,
    task: impl FnOnce() -> Result<T, BoxError> + Send + 'static,
) -> Result<T, BoxError> {
    fiber
        .call(task)
        .await
        .map_err(|err| unknown_error(format!("read fiber unavailable: {err:#}")))?
}

async fn finish_read<T>(
    context: &ReadContextState,
    result: Result<T, BoxError>,
) -> Result<T, BoxError> {
    if result.is_err() {
        let _ = close_context(context).await;
    }
    result
}

/// This resource owns an application read view, created and destroyed on TX.
pub struct ReadContextImpl {
    state: Arc<ReadContextState>,
}

impl Drop for ReadContextImpl {
    fn drop(&mut self) {
        if let Some((fiber, iterators)) = self.state.detach() {
            let id = self.state.id;
            let fallback = iterators.clone();
            if fiber
                .submit(Box::new(move || {
                    let _ = cleanup_view(id, iterators);
                }))
                .is_err()
            {
                run_on_tx_detached(move || {
                    let _ = cleanup_view(id, fallback);
                });
            }
        }
    }
}

pub struct ReadIteratorImpl {
    id: u64,
    context: Arc<ReadContextState>,
}

impl Drop for ReadIteratorImpl {
    fn drop(&mut self) {
        let id = self.id;
        if let Ok(fiber) = self.context.fiber() {
            if fiber.submit(Box::new(move || cleanup_iterator(id))).is_ok() {
                return;
            }
        }
        run_on_tx_detached(move || cleanup_iterator(id));
    }
}

impl SyncWasmStates {
    fn read_context(
        &self,
        handle: &Resource<experimental_read::ReadContext>,
    ) -> Result<Arc<ReadContextState>, BoxError> {
        let state = self.state.lock().unwrap();
        state
            .table
            .get(handle)
            .map(|context: &ReadContextImpl| Arc::clone(&context.state))
            .map_err(|err| unknown_error(format!("invalid read context: {err}")))
    }
}

impl experimental_read::Host for SyncWasmStates {
    async fn open(&mut self) -> Result<Resource<ReadContextImpl>, BoxError> {
        let fiber = TxFiber::spawn()
            .map_err(|err| unknown_error(format!("failed to start read fiber: {err:#}")))?;
        let id = NEXT_READ_ID.fetch_add(1, Ordering::Relaxed);
        let canceled = Arc::new(AtomicBool::new(false));
        let mut pending = PendingOpen {
            id,
            canceled: Arc::clone(&canceled),
            armed: true,
        };
        let opened = call_on_read_fiber(Arc::clone(&fiber), move || {
            let result = with_read_lua(|lua, bridge| {
                let snapshot: Table = bridge.get::<Function>("open")?.call(())?;
                if let Err(err) = lua.set_named_registry_value(&view_key(id), snapshot.clone()) {
                    let _ = bridge.get::<Function>("close")?.call::<()>(snapshot);
                    return Err(err);
                }
                Ok(())
            });
            // A canceled guest may have dropped the response receiver
            // after submitting this job. In that case no resource handle
            // exists to own the newly registered view.
            if canceled.load(Ordering::Acquire) {
                let _ = cleanup_view(id, Vec::new());
            }
            result
        })
        .await;
        opened?;
        pending.armed = false;
        let context = ReadContextImpl {
            state: Arc::new(ReadContextState {
                id,
                fiber: Mutex::new(Some(Arc::clone(&fiber))),
                iterators: Mutex::new(Vec::new()),
            }),
        };
        let mut state = self.state.lock().unwrap();
        state
            .table
            .push(context)
            .map_err(|err| unknown_error(format!("resource table error: {err}")))
    }

    async fn space_by_name(
        &mut self,
        ctx: Resource<experimental_read::ReadContext>,
        name: String,
    ) -> Result<Option<Space>, BoxError> {
        let context = self.read_context(&ctx)?;
        let fiber = context.fiber()?;
        let id = context.id;
        let result = call_on_read_fiber(fiber, move || {
            with_read_lua(|lua, bridge| {
                let found: Option<u32> = bridge
                    .get::<Function>("space_by_name")?
                    .call((view(lua, id)?, name))?;
                Ok(found.map(|id| Space { id }))
            })
        })
        .await;
        finish_read(&context, result).await
    }

    async fn index_by_name(
        &mut self,
        ctx: Resource<experimental_read::ReadContext>,
        space: Space,
        name: String,
    ) -> Result<Option<Index>, BoxError> {
        let context = self.read_context(&ctx)?;
        let fiber = context.fiber()?;
        let id = context.id;
        let result = call_on_read_fiber(fiber, move || {
            with_read_lua(|lua, bridge| {
                let found: Option<u32> = bridge.get::<Function>("index_by_name")?.call((
                    view(lua, id)?,
                    space.id,
                    name,
                ))?;
                Ok(found.map(|id| Index {
                    id,
                    space_id: space.id,
                }))
            })
        })
        .await;
        finish_read(&context, result).await
    }

    async fn get(
        &mut self,
        ctx: Resource<experimental_read::ReadContext>,
        index: Index,
        key: Vec<u8>,
    ) -> Result<Option<BoxTuple>, BoxError> {
        let context = self.read_context(&ctx)?;
        finish_read(&context, validate_array(&key)).await?;
        let fiber = context.fiber()?;
        let id = context.id;
        let result = call_on_read_fiber(fiber, move || {
            with_read_lua(|lua, bridge| {
                let encoded: Option<mlua::LuaString> = bridge.get::<Function>("get")?.call((
                    view(lua, id)?,
                    index.space_id,
                    index.id,
                    lua.create_string(&key)?,
                ))?;
                Ok(encoded.map(|bytes| {
                    tuple_ptr(unsafe { Tuple::from_slice(bytes.as_bytes().as_ref()) })
                }))
            })
        })
        .await;
        let result = result.and_then(|ptr| ptr.map(|ptr| self.register_tuple(ptr)).transpose());
        finish_read(&context, result).await
    }

    async fn count(
        &mut self,
        ctx: Resource<experimental_read::ReadContext>,
        index: Index,
        mode: IteratorType,
        key: Vec<u8>,
    ) -> Result<u64, BoxError> {
        let context = self.read_context(&ctx)?;
        finish_read(&context, validate_array(&key)).await?;
        let mode = finish_read(&context, convert_iterator_type(mode)).await? as i32;
        let fiber = context.fiber()?;
        let id = context.id;
        let result = call_on_read_fiber(fiber, move || {
            with_read_lua(|lua, bridge| {
                bridge.get::<Function>("count")?.call((
                    view(lua, id)?,
                    index.space_id,
                    index.id,
                    mode,
                    lua.create_string(&key)?,
                ))
            })
        })
        .await;
        finish_read(&context, result).await
    }

    async fn select(
        &mut self,
        ctx: Resource<experimental_read::ReadContext>,
        index: Index,
        mode: IteratorType,
        key: Vec<u8>,
    ) -> Result<Resource<ReadIteratorImpl>, BoxError> {
        let context = self.read_context(&ctx)?;
        finish_read(&context, validate_array(&key)).await?;
        let mode = finish_read(&context, convert_iterator_type(mode)).await? as i32;
        let fiber = context.fiber()?;
        let id = context.id;
        let iterator_id = NEXT_READ_ID.fetch_add(1, Ordering::Relaxed);
        // Track the registry slot before the async job. If the guest is
        // canceled while awaiting it, context drop still clears this slot.
        context.iterators.lock().unwrap().push(iterator_id);
        let result = call_on_read_fiber(fiber, move || {
            with_read_lua(|lua, bridge| {
                let next: Function = bridge.get::<Function>("select")?.call((
                    view(lua, id)?,
                    index.space_id,
                    index.id,
                    mode,
                    lua.create_string(&key)?,
                ))?;
                lua.set_named_registry_value(&iterator_key(iterator_id), next)
            })
        })
        .await;
        finish_read(&context, result).await?;
        let iterator = ReadIteratorImpl {
            id: iterator_id,
            context: Arc::clone(&context),
        };
        let result = {
            let mut state = self.state.lock().unwrap();
            state
                .table
                .push(iterator)
                .map_err(|err| unknown_error(format!("resource table error: {err}")))
        };
        finish_read(&context, result).await
    }
}

impl experimental_read::HostReadContext for SyncWasmStates {
    async fn close(
        &mut self,
        self_: Resource<experimental_read::ReadContext>,
    ) -> Result<(), BoxError> {
        let context = self.read_context(&self_)?;
        close_context(&context).await
    }

    async fn drop(
        &mut self,
        self_: Resource<experimental_read::ReadContext>,
    ) -> Result<(), wasmtime::Error> {
        let context = self.state.lock().unwrap().table.delete(self_)?;
        drop(context);
        Ok(())
    }
}

impl experimental_read::HostIterator for SyncWasmStates {
    async fn next(
        &mut self,
        self_: Resource<experimental_read::Iterator>,
        ctx: Resource<experimental_read::ReadContext>,
    ) -> Result<Option<BoxTuple>, BoxError> {
        let (iterator_id, context) = {
            let state = self.state.lock().unwrap();
            let iterator: &ReadIteratorImpl = state
                .table
                .get(&self_)
                .map_err(|err| unknown_error(format!("invalid read iterator: {err}")))?;
            (iterator.id, Arc::clone(&iterator.context))
        };
        let supplied_context = self.read_context(&ctx)?;
        if !Arc::ptr_eq(&context, &supplied_context) {
            return Err(unknown_error(
                "iterator belongs to a different read context",
            ));
        }
        let fiber = context.fiber()?;
        let result = call_on_read_fiber(fiber, move || {
            with_read_lua(|lua, _| {
                let next: Option<Function> =
                    lua.named_registry_value(&iterator_key(iterator_id))?;
                let next = next.ok_or_else(|| mlua::Error::runtime("read iterator is closed"))?;
                let encoded: Option<mlua::LuaString> = next.call(())?;
                Ok(encoded.map(|bytes| {
                    tuple_ptr(unsafe { Tuple::from_slice(bytes.as_bytes().as_ref()) })
                }))
            })
        })
        .await;
        let result = result.and_then(|ptr| ptr.map(|ptr| self.register_tuple(ptr)).transpose());
        finish_read(&context, result).await
    }

    async fn drop(
        &mut self,
        self_: Resource<experimental_read::Iterator>,
    ) -> Result<(), wasmtime::Error> {
        let iterator = self.state.lock().unwrap().table.delete(self_)?;
        drop(iterator);
        Ok(())
    }
}
