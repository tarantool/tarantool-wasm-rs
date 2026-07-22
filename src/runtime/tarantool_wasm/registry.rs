use super::*;

#[derive(Clone)]
pub(super) struct LoadedModule {
    pub(super) component: Component,
    pub(super) config: WasmConfig,
    pub(super) fuel: bool,
    pub(super) export_cache: Arc<RwLock<HashMap<ExportCacheKey, ComponentExportIndex>>>,
    pub(super) mode: ModuleMode,
}

#[derive(Clone)]
struct CachedComponent {
    source: Arc<[u8]>,
    component: Component,
}

const COMPONENT_CACHE_CAPACITY: usize = 32;

#[derive(Default)]
pub(super) struct ComponentCache {
    entries: HashMap<(PathBuf, bool), CachedComponent>,
    insertion_order: VecDeque<(PathBuf, bool)>,
}

impl ComponentCache {
    fn insert(&mut self, key: (PathBuf, bool), component: CachedComponent) {
        if let std::collections::hash_map::Entry::Occupied(mut entry) =
            self.entries.entry(key.clone())
        {
            entry.insert(component);
            return;
        }

        if self.entries.len() == COMPONENT_CACHE_CAPACITY {
            if let Some(oldest) = self.insertion_order.pop_front() {
                self.entries.remove(&oldest);
            }
        }

        self.insertion_order.push_back(key.clone());
        self.entries.insert(key, component);
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }
}

#[derive(Clone)]
pub(super) enum ModuleMode {
    Regular,
    Interpreter(Arc<SharedInterpreterEntry>),
}

pub(super) struct RuntimeState {
    pub(super) tx_thread_id: ThreadId,
    pub engine: Engine,
    pub linker: Arc<Linker<SyncWasmStates>>,
    pub fuel_engine: Engine,
    pub fuel_linker: Arc<Linker<SyncWasmStates>>,
    pub components: RwLock<ComponentCache>,
    pub modules: RwLock<HashMap<String, LoadedModule>>,
    pub shared_interpreters: RwLock<HashMap<String, Arc<SharedInterpreterEntry>>>,
    pub sessions: RwLock<HashMap<String, Arc<WasmSession>>>,
    pub runs: RwLock<HashMap<u64, Arc<WasmRunEntry>>>,
    pub next_uid: AtomicU64,
    pub next_session_uid: AtomicU64,
    pub next_run_uid: AtomicU64,
}

impl RuntimeState {
    pub(super) fn new() -> Self {
        let engine = Engine::new(&runtime_config(false)).unwrap();
        let fuel_engine = Engine::new(&runtime_config(true)).unwrap();

        let linker = build_runtime_linker(&engine);
        let fuel_linker = build_runtime_linker(&fuel_engine);

        let components = RwLock::new(ComponentCache::default());
        let modules = RwLock::new(HashMap::new());
        let shared_interpreters = RwLock::new(HashMap::new());
        let sessions = RwLock::new(HashMap::new());
        let runs = RwLock::new(HashMap::new());
        let linker_ref = Arc::new(linker);
        let fuel_linker_ref = Arc::new(fuel_linker);

        let _ = on_shutdown(|| {
            if let Err(err) = shutdown_runtime() {
                eprintln!("failed to stop WASM runtime cleanly: {err}");
            }
        });

        RuntimeState {
            tx_thread_id: thread::current().id(),
            engine,
            linker: linker_ref,
            fuel_engine,
            fuel_linker: fuel_linker_ref,
            components,
            modules,
            shared_interpreters,
            sessions,
            runs,
            next_uid: AtomicU64::new(0),
            next_session_uid: AtomicU64::new(0),
            next_run_uid: AtomicU64::new(0),
        }
    }
}

fn load_component(path: &Path, use_fuel: bool) -> Result<Component> {
    let source: Arc<[u8]> = fs::read(path)
        .map_err(|err| anyhow!("failed to read component `{}`: {err}", path.display()))?
        .into();
    let cache_key = (path.to_path_buf(), use_fuel);

    if let Some(cached) = runtime_state
        .components
        .read()
        .unwrap()
        .entries
        .get(&cache_key)
    {
        if cached.source == source {
            return Ok(cached.component.clone());
        }
    }

    let engine = if use_fuel {
        &runtime_state.fuel_engine
    } else {
        &runtime_state.engine
    };
    let component = Component::from_binary(engine, &source)?;

    let mut components = runtime_state.components.write().unwrap();
    if let Some(cached) = components.entries.get(&cache_key) {
        if cached.source == source {
            return Ok(cached.component.clone());
        }
    }
    components.insert(
        cache_key,
        CachedComponent {
            source,
            component: component.clone(),
        },
    );

    Ok(component)
}

/// Load a WebAssembly component from disk and register it in the runtime,
/// returning a unique module identifier.
pub fn tt_wasm_load_module(path: String, config: WasmConfig) -> Result<String> {
    lifecycle::ensure_running()?;
    let use_fuel = config.max_instructions.is_some();
    let canonical_path = PathBuf::from(&path)
        .canonicalize()
        .map_err(|err| anyhow!("failed to canonicalize `{path}`: {err}"))?;

    let loaded = if config.interpreter {
        let path_key = canonical_path.to_string_lossy().to_string();

        let entry = {
            let mut registry = runtime_state.shared_interpreters.write().unwrap();
            if let Some(existing) = registry.get(&path_key) {
                if existing.config != config {
                    return Err(anyhow!(
                        "interpreter `{path_key}` already loaded with a different config"
                    ));
                }
                existing.refcount.fetch_add(1, Ordering::Relaxed);
                Arc::clone(existing)
            } else {
                let component = load_component(&canonical_path, use_fuel)?;
                let export_cache = Arc::new(RwLock::new(HashMap::new()));
                let (cmd_tx, cmd_rx) = std_mpsc::channel();
                let (ready_tx, ready_rx) = std_mpsc::channel();

                let worker = spawn_tt_thread("wasm-interpreter".to_string(), {
                    let path_key_for_thread = path_key.clone();
                    let component_for_thread = component.clone();
                    let config_for_thread = config.clone();
                    let export_cache_for_thread = Arc::clone(&export_cache);
                    move || {
                        run_interpreter_worker(
                            path_key_for_thread,
                            component_for_thread,
                            config_for_thread,
                            use_fuel,
                            export_cache_for_thread,
                            ready_tx,
                            cmd_rx,
                        )
                    }
                })?;

                if let Err(err) = recv_result_via_coio(ready_rx) {
                    let _ = join_handle_via_coio(worker);
                    return Err(err);
                }
                if let Err(err) = lifecycle::ensure_running() {
                    let (done_tx, _done_rx) = std_mpsc::channel();
                    let _ = cmd_tx.send(InterpreterCommand::Close(done_tx));
                    let _ = join_handle_via_coio(worker);
                    return Err(err);
                }

                let created = Arc::new(SharedInterpreterEntry {
                    path_key: path_key.clone(),
                    component: component.clone(),
                    config: config.clone(),
                    fuel: use_fuel,
                    export_cache: Arc::clone(&export_cache),
                    cmd_tx,
                    join_handle: Mutex::new(Some(worker)),
                    refcount: AtomicU64::new(1),
                });
                registry.insert(path_key.clone(), Arc::clone(&created));
                created
            }
        };

        LoadedModule {
            component: entry.component.clone(),
            config: entry.config.clone(),
            fuel: entry.fuel,
            export_cache: Arc::clone(&entry.export_cache),
            mode: ModuleMode::Interpreter(entry),
        }
    } else {
        let component = load_component(&canonical_path, use_fuel)?;
        LoadedModule {
            component,
            config,
            fuel: use_fuel,
            export_cache: Arc::new(RwLock::new(HashMap::new())),
            mode: ModuleMode::Regular,
        }
    };

    lifecycle::ensure_running()?;
    let uid = format!(
        "{}#{}",
        path,
        runtime_state.next_uid.fetch_add(1, Ordering::Relaxed)
    );
    runtime_state
        .modules
        .write()
        .unwrap()
        .insert(uid.clone(), loaded);
    Ok(uid)
}

/// Drop a previously loaded module, freeing associated resources.
pub fn tt_wasm_drop_module(module_uid: String) -> Result<()> {
    lifecycle::ensure_running()?;
    let mode = {
        let mut modules = runtime_state.modules.write().unwrap();
        let loaded = modules
            .remove(&module_uid)
            .ok_or_else(|| anyhow::anyhow!("module `{module_uid}` not found"))?;
        loaded.mode
    };

    let mut sessions_to_close = Vec::new();
    {
        let mut sessions = runtime_state.sessions.write().unwrap();
        sessions.retain(|session_handle, session| {
            if session.module_uid == module_uid {
                sessions_to_close.push((session_handle.clone(), Arc::clone(session)));
                false
            } else {
                true
            }
        });
    }

    for (session_handle, session) in sessions_to_close {
        close_session_impl(&session_handle, session)?;
    }

    if let ModuleMode::Interpreter(entry) = mode {
        let previous = entry.refcount.fetch_sub(1, Ordering::Relaxed);
        if previous == 0 {
            entry.refcount.store(0, Ordering::Relaxed);
            return Err(anyhow!(
                "interpreter refcount underflow for path `{}`",
                entry.path_key
            ));
        }

        if previous == 1 {
            let path_key = entry.path_key.clone();
            {
                let mut registry = runtime_state.shared_interpreters.write().unwrap();
                if registry
                    .get(&entry.path_key)
                    .map(|current| Arc::ptr_eq(current, &entry))
                    .unwrap_or(false)
                {
                    registry.remove(&entry.path_key);
                }
            }
            close_interpreter_impl(&path_key, entry)?;
        }
    }

    Ok(())
}

#[derive(Debug, serde::Serialize)]
pub struct ExportInfo {
    pub params: Vec<(String, String)>,
    pub results: Vec<String>,
}

type ExportNamespace = HashMap<String, ExportInfo>;
pub(super) type ExportMap = HashMap<String, ExportNamespace>;

/// Retrieve a map of exported functions grouped by namespace.
pub fn tt_wasm_export_names(module_uid: String) -> Result<ExportMap> {
    lifecycle::ensure_running()?;
    let loaded = runtime_state
        .modules
        .read()
        .unwrap()
        .get(&module_uid)
        .ok_or_else(|| anyhow::anyhow!("module `{}` not found", module_uid))?
        .clone();

    let component = loaded.component;
    let ty = component.component_type();
    let engine = if loaded.fuel {
        &runtime_state.fuel_engine
    } else {
        &runtime_state.engine
    };

    let mut out: ExportMap = HashMap::new();

    for (name, item) in ty.exports(engine) {
        match item.ty {
            ComponentItem::ComponentFunc(f) => {
                out.entry("".to_string())
                    .or_default()
                    .insert(name.to_string(), export_info_from_component_func(f));
            }

            ComponentItem::ComponentInstance(inst) => {
                if name == "exports" {
                    // Exclude the `exports` instance — it is an internal interface used for module setup
                    // and is not intended to be invoked directly from Lua.
                    continue;
                }

                if name.starts_with("wasi") {
                    // Exclude `wasi:*` instances — they provide runtime support (e.g., `wasi:cli/run`)
                    // and are not designed to be called manually from user code.
                    continue;
                }
                collect_exports(engine, &inst, name, &mut out);
            }
            _ => {}
        }
    }

    Ok(out)
}
