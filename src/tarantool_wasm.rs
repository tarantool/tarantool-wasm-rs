use std::path::PathBuf;
use wasmtime::component::{ResourceTable};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use wasmtime::{Engine, Linker, Store, Module};
use anyhow::Result;
use lazy_static::lazy_static;
use std::sync::Mutex;
use std::thread;

lazy_static! {
    static ref runtime_state: RuntimeState = RuntimeState::new();
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
}

impl RuntimeState {
    pub fn new() -> Self {
        let engine = Engine::default();
        let mut linker = wasmtime::component::Linker::<SyncWasmStates>::new(&engine);
        let mut linker_preview1 = Linker::<WasiP1Ctx>::new(&engine);
        preview1::add_to_linker_sync(&mut linker_preview1, |t| t);
        wasmtime_wasi::add_to_linker_sync(&mut linker);
        RuntimeState { engine, linker, linker_preview1 }
    }
}

pub fn tt_wasm_load_module(path: PathBuf) -> Result<Module> {
    print!("The path is {}", path.display());
    Module::from_file(&(runtime_state.engine), path)
}

pub fn tt_wasm_run_module(module: &Module) -> Result<()> {
        let mut ctx_builder =  WasiCtxBuilder::new();
        let ctx = ctx_builder.build_p1();
        let mut store = Store::new(&runtime_state.engine, ctx);
        let instance = runtime_state.linker_preview1.instantiate(&mut store, &module)?;

        let handle = thread::spawn(move || -> Result<()> {
            let func = instance.get_typed_func::<(), ()>(&mut store, "_start")?;
            func.call(&mut store, ())?;
            Ok(())
        });
        handle.join().unwrap()
}
