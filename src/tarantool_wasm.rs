use wasmtime::component::{ResourceTable};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use wasmtime::{Engine, Linker, Store, Module, TypedFunc, Instance};
use anyhow::Result;
use lazy_static::lazy_static;
use std::thread;
use std::collections::HashMap;
use std::sync::{Mutex, Arc};
use anyhow::anyhow;

lazy_static! {
    static ref runtime_state: Arc<Mutex<RuntimeState>> = Arc::new(Mutex::new(RuntimeState::new()));
}

pub struct WasmStates {
    table: ResourceTable,
    ctx: WasiCtx,
}

struct SyncWasmStates(Mutex<WasmStates>);

impl WasmStates {
    pub fn new() -> Self {
        let table = ResourceTable::new();
        let mut builder = WasiCtxBuilder::new();
        // TODO: make env and stdio configurable
        builder.inherit_stdio().inherit_env();
        let ctx = builder.build();
        Self { table, ctx }
    }
}

impl SyncWasmStates {
    pub fn new() -> Self {
        SyncWasmStates(Mutex::new(WasmStates::new()))
    }
}

impl WasiView for SyncWasmStates {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.0.get_mut().unwrap().table
    }

    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.0.get_mut().unwrap().ctx
    }
}

struct RuntimeState {
    pub engine: Engine,
    pub linker: wasmtime::component::Linker<SyncWasmStates>,
    pub linker_preview1: Linker<WasiP1Ctx>,
    pub modules: HashMap<String, Module>
}

impl RuntimeState {
    pub fn new() -> Self {
        let engine = Engine::default();
        let mut linker = wasmtime::component::Linker::<SyncWasmStates>::new(&engine);
        let mut linker_preview1 = Linker::<WasiP1Ctx>::new(&engine);
        preview1::add_to_linker_sync(&mut linker_preview1, |t| t);
        wasmtime_wasi::add_to_linker_sync(&mut linker);
        let modules = HashMap::new();
        RuntimeState { engine, linker, linker_preview1, modules }
    }
}

pub fn tt_wasm_load_module(path: String) -> Result<String> {
    let mut state = runtime_state.lock().unwrap();
    match Module::from_file(&state.engine, path.clone()) {
        Ok(module) => {
            state.modules.insert(path.clone(), module);
            return Ok(path);
        },
        Err(e) => return Err(e),
    }
}

fn get_entry_point(instance: &Instance, store: &mut Store<WasiP1Ctx>) -> Result<TypedFunc<(), ()>> {
    let names = ["_start", "main", "_main", "__main_argc_argv"];
    for item in names.into_iter() {
        match instance.get_typed_func::<(), ()>(&mut *store, item) {
            Ok(func) => {
                println!("{}", item);
                return Ok(func)
            },
            Err(e) => {}
        };
    }
    Err(anyhow!("Entry point not found"))
}

pub fn tt_wasm_run_module(module_uid: String) -> Result<()> {
    let state = runtime_state.lock().unwrap();
    let module = state.modules.get(&module_uid).unwrap();
    let mut ctx_builder =  WasiCtxBuilder::new();
    let ctx = ctx_builder.inherit_stdio().build_p1();
    let mut store = Store::new(&state.engine, ctx);
    let instance = state.linker_preview1.instantiate(&mut store, &module)?;
    let func = get_entry_point(&instance, &mut store)?;
    match func.call(&mut store, ()) {
        Ok(()) => Ok(()),
        Err(e) => Err(e),
    }
}
