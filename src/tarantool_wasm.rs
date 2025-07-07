use crate::bindings::tarantool::tarantool::{
    box_tuple, error, index, key_def, msgpack, say, sequence, session, ttbox, tuple_format, txn,
    types,
};
use crate::executor::{Executor, TxMode};
use crate::wasm_fiber::WasmFiber;
use crate::wasm_fiber::WasmFiberStatus;
use anyhow::Result;
use lazy_static::lazy_static;
use nix::sys::signal::{SigSet, Signal};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use tarantool::fiber;
use tarantool::trigger::on_shutdown;
use tokio::sync::mpsc;
use wasmtime::component::Component;
use wasmtime::component::{HasData, Linker, ResourceTable};
use wasmtime::{AsContext, Config};
use wasmtime::{Engine, Store};
use wasmtime_wasi::p2::{IoView, WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};
use wasmtime_wasi_tls::{LinkOptions, WasiTlsCtx};

use crate::bindings::docs::adder::{wlua, zlib};

lazy_static! {
    static ref runtime_state: RuntimeState = RuntimeState::new();
}

pub(crate) struct WasmStates {
    pub(crate) table: ResourceTable,
    pub(crate) ctx: WasiCtx,
    pub(crate) http: WasiHttpCtx,
    pub(crate) yielder: Option<mpsc::Sender<WasmFiberStatus>>,
    pub(crate) executor: Executor,
    pub(crate) tx_mode: TxMode,
}

impl WasmStates {
    pub fn new() -> Self {
        let table = ResourceTable::new();
        let mut builder = WasiCtxBuilder::new();
        // TODO: make env and stdio configurable
        builder
            .inherit_stdio()
            .inherit_env()
            .inherit_network()
            .allow_ip_name_lookup(true);

        let ctx = builder.build();
        Self {
            table: table,
            ctx: ctx,
            http: WasiHttpCtx::new(),
            yielder: None,
            executor: Executor::TxPush, // start outside the transaction
            tx_mode: TxMode::None,
        }
    }
}

pub(crate) struct SyncWasmStates(pub(crate) Mutex<WasmStates>);

impl SyncWasmStates {
    pub(crate) fn new() -> Self {
        SyncWasmStates(Mutex::new(WasmStates::new()))
    }

    pub fn begin_tx(&mut self) {
        self.0.lock().unwrap().tx_mode = TxMode::Started;
        self.0.lock().unwrap().executor = Executor::Direct;
    }

    pub fn end_tx(&mut self) {
        self.0.lock().unwrap().tx_mode = TxMode::None;
        self.0.lock().unwrap().executor = Executor::TxPush;
    }

    pub fn executor(&self) -> Executor {
        self.0.lock().unwrap().executor
    }

    pub(crate) async fn yield_now(&self) {
        let yielder_handle = self.0.lock().unwrap().yielder.clone().unwrap();
        let _ = yielder_handle.send(WasmFiberStatus::Suspended).await;
        tokio::task::yield_now().await;
    }
}

impl WasiView for SyncWasmStates {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.0.get_mut().unwrap().ctx
    }
}

impl IoView for SyncWasmStates {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.0.get_mut().unwrap().table
    }
}

impl WasiHttpView for SyncWasmStates {
    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.0.get_mut().unwrap().http
    }
}

impl HasData for SyncWasmStates {
    type Data<'a> = &'a mut SyncWasmStates;
}

struct RuntimeState {
    pub engine: Engine,
    pub linker: Arc<Linker<SyncWasmStates>>,
    pub modules: RwLock<HashMap<String, Component>>,
}

fn get_configuration() -> Config {
    let mut config = Config::new();
    /*
     * Async support is required to implement rescheduling into the TX
     * thread. In non-async mode WASM is heavily dependent on the
     * thread-local storage. In async mode it changes to the task-local
     * storage. This way we can run our module within a Tokio runtime
     * task and reschedule that task instead.
     */
    config.async_support(true);

    /*
     * We use epoch-based interruptions to implement graceful shutdown.
     * Epoch is a global tick counter managed by the engine. We can setup
     * a limiting epoch for each module, after which it will halt. With
     * that technique we can set the limit at 1 tick for all modules and
     * then halt all modules at once by incrementing the epoch.
     */
    config.epoch_interruption(true);
    config
}

impl RuntimeState {
    pub fn new() -> Self {
        let config = get_configuration();
        let engine = Engine::new(&config).unwrap();
        let mut linker = Linker::<SyncWasmStates>::new(&engine);
        let _ = wasmtime_wasi::p2::add_to_linker_async(&mut linker);
        let _ = wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker);

        let mut opts = LinkOptions::default();
        opts.tls(true);
        let _ =
            wasmtime_wasi_tls::add_to_linker(&mut linker, &mut opts, |h: &mut SyncWasmStates| {
                WasiTlsCtx::new(&mut h.0.get_mut().unwrap().table)
            });

        let _ = ttbox::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
        let _ = say::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
        let _ = error::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
        let _ =
            session::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
        let _ =
            msgpack::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
        let _ = txn::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
        let _ = types::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
        let _ = index::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
        let _ =
            sequence::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
        let _ =
            key_def::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
        let _ =
            tuple_format::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| {
                state
            });
        let _ =
            box_tuple::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
        let _ = zlib::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);
        let _ = wlua::add_to_linker::<SyncWasmStates, SyncWasmStates>(&mut linker, |state| state);

        let modules = RwLock::new(HashMap::new());
        let linker_ref = Arc::new(linker);

        let engine_ref = engine.clone();

        let _ = on_shutdown(move || {
            engine_ref.increment_epoch();
            fiber::sleep(Duration::new(1, 0));
        });

        RuntimeState {
            engine,
            linker: linker_ref,
            modules,
        }
    }
}

pub fn tt_wasm_load_module(path: String) -> Result<String> {
    match Component::from_file(&runtime_state.engine, path.clone()) {
        Ok(module) => {
            runtime_state
                .modules
                .write()
                .unwrap()
                .insert(path.clone(), module);
            Ok(path)
        }
        Err(e) => Err(e),
    }
}

fn spawn_tt_thread<F, T>(name: String, f: F) -> Result<JoinHandle<T>>
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

pub fn tt_wasm_run_module(module_uid: String) -> Result<JoinHandle<Result<()>>> {
    let handle = spawn_tt_thread("wasm".to_string(), move || {
        let component = runtime_state
            .modules
            .read()
            .unwrap()
            .get(&module_uid)
            .unwrap()
            .clone();
        let linker = runtime_state.linker.clone();

        let wasi_view = SyncWasmStates::new();
        let mut store = Store::new(&runtime_state.engine, wasi_view);

        store.set_epoch_deadline(1);
        store.epoch_deadline_trap();

        let fiber = WasmFiber::new(store, linker, component);
        let _ = fiber.block_on();
        Ok::<(), wasmtime::Error>(())
    })?;
    Ok(handle)
}

use rmpv::decode::read_value;
use rmpv::encode::write_value;
use rmpv::Value as RmpvValue;
use std::io::Cursor;
use tokio::runtime::Builder;
use wasmtime::component::Type;
use wasmtime::component::Val;

fn to_component_val(ty: &Type, val: &RmpvValue) -> Result<Val> {
    Ok(match ty {
        // Signed integers
        Type::S8 => Val::S8(val.as_i64().ok_or_else(|| anyhow::anyhow!("expected s8"))? as i8),
        Type::S16 => Val::S16(
            val.as_i64()
                .ok_or_else(|| anyhow::anyhow!("expected s16"))? as i16,
        ),
        Type::S32 => Val::S32(
            val.as_i64()
                .ok_or_else(|| anyhow::anyhow!("expected s32"))? as i32,
        ),
        Type::S64 => Val::S64(
            val.as_i64()
                .ok_or_else(|| anyhow::anyhow!("expected s64"))? as i64,
        ),

        // Unsigned integers
        Type::U8 => Val::U8(val.as_u64().ok_or_else(|| anyhow::anyhow!("expected u8"))? as u8),
        Type::U16 => Val::U16(
            val.as_u64()
                .ok_or_else(|| anyhow::anyhow!("expected u16"))? as u16,
        ),
        Type::U32 => Val::U32(
            val.as_u64()
                .ok_or_else(|| anyhow::anyhow!("expected u32"))? as u32,
        ),
        Type::U64 => Val::U64(
            val.as_u64()
                .ok_or_else(|| anyhow::anyhow!("expected u64"))? as u64,
        ),

        // Floats
        Type::Float32 => {
            let f = val
                .as_f64()
                .ok_or_else(|| anyhow::anyhow!("expected f32"))? as f32;
            Val::Float32(f)
        }
        Type::Float64 => {
            let f = val
                .as_f64()
                .ok_or_else(|| anyhow::anyhow!("expected f64"))?;
            Val::Float64(f)
        }

        // Boolean
        Type::Bool => Val::Bool(val.as_bool().unwrap_or(false)),

        // String
        Type::String => {
            let s = val
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("expected string"))?;
            Val::String(s.to_string())
        }

        // Char
        Type::Char => {
            let s = val
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("expected char string"))?;
            let mut chars = s.chars();
            let c = chars
                .next()
                .ok_or_else(|| anyhow::anyhow!("empty string for char"))?;
            if chars.next().is_some() {
                return Err(anyhow::anyhow!("expected single character string"));
            }
            Val::Char(c)
        }

        // List
        Type::List(list_ty) => {
            let elem_ty = list_ty.ty();
            let arr = val
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("expected array for list"))?;
            let list = arr
                .iter()
                .map(|v| to_component_val(&elem_ty, v))
                .collect::<Result<Vec<_>>>()?;
            Val::List(list.into())
        }

        _ => return Err(anyhow::anyhow!("unsupported param type: {:?}", ty)),
    })
}

pub fn val_to_rmpv(val: &Val) -> Result<RmpvValue> {
    Ok(match val {
        Val::S8(i) => RmpvValue::Integer((*i as i64).into()),
        Val::U8(i) => RmpvValue::Integer((*i as i64).into()),
        Val::S16(i) => RmpvValue::Integer((*i as i64).into()),
        Val::U16(i) => RmpvValue::Integer((*i as i64).into()),
        Val::S32(i) => RmpvValue::Integer((*i as i64).into()),
        Val::U32(i) => RmpvValue::Integer((*i as i64).into()),
        Val::S64(i) => RmpvValue::Integer((*i).into()),
        Val::U64(i) => RmpvValue::Integer((*i as i64).into()),
        Val::Float32(f) => RmpvValue::F64(*f as f64),
        Val::Float64(f) => RmpvValue::F64(*f),
        Val::Bool(b) => RmpvValue::Boolean(*b),
        Val::Char(c) => RmpvValue::String(c.to_string().into()),
        Val::String(s) => RmpvValue::String(s.clone().into()),
        Val::Enum(s) => RmpvValue::String(s.clone().into()),

        Val::List(list) => {
            let items = list.iter().map(val_to_rmpv).collect::<Result<Vec<_>>>()?;
            RmpvValue::Array(items)
        }
        Val::Tuple(items) => {
            let values = items.iter().map(val_to_rmpv).collect::<Result<Vec<_>>>()?;
            RmpvValue::Array(values)
        }
        Val::Record(fields) => {
            let map = fields
                .iter()
                .map(|(k, v)| Ok((RmpvValue::String(k.clone().into()), val_to_rmpv(v)?)))
                .collect::<Result<Vec<_>>>()?;
            RmpvValue::Map(map)
        }
        Val::Variant(discriminant, value) => {
            let name = discriminant.clone();
            let v = if let Some(val) = value.as_ref() {
                val_to_rmpv(val)?
            } else {
                RmpvValue::Nil
            };
            RmpvValue::Map(vec![(RmpvValue::String(name.into()), v)])
        }
        Val::Option(opt) => match opt {
            Some(inner) => RmpvValue::Array(vec![val_to_rmpv(inner)?]),
            None => RmpvValue::Nil,
        },
        Val::Result(r) => match r {
            Ok(Some(v)) => RmpvValue::Map(vec![(RmpvValue::String("Ok".into()), val_to_rmpv(v)?)]),
            Ok(None) => RmpvValue::Map(vec![(RmpvValue::String("Ok".into()), RmpvValue::Nil)]),
            Err(Some(e)) => {
                RmpvValue::Map(vec![(RmpvValue::String("Err".into()), val_to_rmpv(e)?)])
            }
            Err(None) => RmpvValue::Map(vec![(RmpvValue::String("Err".into()), RmpvValue::Nil)]),
        },
        Val::Flags(flags) => {
            let arr = flags
                .iter()
                .map(|s| RmpvValue::String(s.to_string().into()))
                .collect::<Vec<_>>();
            RmpvValue::Array(arr)
        }
        other => return Err(anyhow::anyhow!("Unsupported Val: {other:?}")),
    })
}

pub fn tt_wasm_call_function(
    module_uid: String,
    func: String,
    msgpack_args: Vec<u8>,
) -> Result<Vec<u8>> {
    let component = runtime_state
        .modules
        .read()
        .unwrap()
        .get(&module_uid)
        .ok_or_else(|| anyhow::anyhow!("module `{}` not found", module_uid))?
        .clone();

    let linker = runtime_state.linker.clone();
    let wasi_view = SyncWasmStates::new();
    let mut store = Store::new(&runtime_state.engine, wasi_view);

    store.set_epoch_deadline(1);
    store.epoch_deadline_trap();

    let rt = Builder::new_current_thread()
        .enable_time()
        .enable_io()
        .build()
        .unwrap();

    rt.block_on(async move {
        let instance = linker.instantiate_async(&mut store, &component).await?;

        let export = instance
            .get_func(&mut store, &func)
            .ok_or_else(|| anyhow::anyhow!(format!("function export `{}` not found", func)))?;

        let params = export.params(&store.as_context());
        let param_types = params.iter().map(|(_, t)| t).collect::<Vec<_>>();

        let mp_value = read_value(&mut Cursor::new(&msgpack_args))?;
        let arr = mp_value
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("msgpack args must be array"))?;

        if arr.len() != param_types.len() {
            return Err(anyhow::anyhow!(
                "argument count mismatch: expected {}, got {}",
                param_types.len(),
                arr.len()
            ));
        }

        let mut wasm_args = Vec::new();
        for (ty, val) in param_types.iter().zip(arr) {
            let v = to_component_val(ty, val)?;
            wasm_args.push(v);
        }

        let mut results_buf = [Val::Bool(false)];

        export
            .call_async(&mut store, &wasm_args, &mut results_buf)
            .await?;

        let res_val: RmpvValue = val_to_rmpv(&results_buf[0])?;

        let mut buf = Vec::new();
        write_value(&mut buf, &res_val)?;
        Ok(buf)
    })
}
