use std::{collections::HashMap, future::Future, pin::Pin};

use anyhow::{anyhow, Result};
use wasmtime::{
    component::{
        types::{ComponentFunc, ComponentItem},
        Component, ComponentExportIndex, Instance, Linker, Val,
    },
    Store, StoreContextMut,
};

use super::{runtime_state, CallJob, LoadedModule, ModuleMode, SyncWasmStates};

type LinkedFuncFuture<'a> = Box<dyn Future<Output = Result<()>> + Send + 'a>;
type LinkedFunc = dyn for<'a> Fn(
        StoreContextMut<'a, SyncWasmStates>,
        ComponentFunc,
        &'a [Val],
        &'a mut [Val],
    ) -> LinkedFuncFuture<'a>
    + Send
    + Sync
    + 'static;

#[derive(Default)]
struct InstanceGraph {
    stack: Vec<String>,
    instances: HashMap<String, Instance>,
}

pub(super) async fn instantiate_linked_module(
    store: &mut Store<SyncWasmStates>,
    module_uid: &str,
    loaded: &LoadedModule,
) -> Result<Instance> {
    let mut graph = InstanceGraph::default();
    instantiate_linked_module_inner(store, module_uid, loaded, &mut graph).await
}

fn instantiate_linked_module_inner<'a>(
    store: &'a mut Store<SyncWasmStates>,
    module_uid: &'a str,
    loaded: &'a LoadedModule,
    graph: &'a mut InstanceGraph,
) -> Pin<Box<dyn Future<Output = Result<Instance>> + Send + 'a>> {
    Box::pin(async move {
        if let Some(instance) = graph.instances.get(module_uid).copied() {
            return Ok(instance);
        }

        if let Some(pos) = graph.stack.iter().position(|item| item == module_uid) {
            let mut cycle = graph.stack[pos..].to_vec();
            cycle.push(module_uid.to_string());
            return Err(anyhow!(
                "component link cycle detected: {}",
                cycle.join(" -> ")
            ));
        }

        graph.stack.push(module_uid.to_string());
        let result = async {
            let mut linker = if loaded.fuel {
                runtime_state.fuel_linker.as_ref().clone()
            } else {
                runtime_state.linker.as_ref().clone()
            };

            register_component_links(store, &mut linker, module_uid, loaded, graph).await?;

            linker
                .instantiate_async(&mut *store, &loaded.component)
                .await
                .map_err(|err| anyhow!("failed to instantiate module `{module_uid}`: {err}"))
        }
        .await;
        graph.stack.pop();

        let instance = result?;
        graph.instances.insert(module_uid.to_string(), instance);
        Ok(instance)
    })
}

fn register_component_links<'a>(
    store: &'a mut Store<SyncWasmStates>,
    linker: &'a mut Linker<SyncWasmStates>,
    module_uid: &'a str,
    loaded: &'a LoadedModule,
    graph: &'a mut InstanceGraph,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let engine = loaded.component.engine();

        for (import_name, provider_uid) in &loaded.config.links {
            let import_item = loaded
                .component
                .component_type()
                .get_import(engine, import_name)
                .ok_or_else(|| {
                    anyhow!(
                        "module `{module_uid}` does not import `{import_name}` required by `links`"
                    )
                })?;

            let provider = runtime_state
                .modules
                .read()
                .unwrap()
                .get(provider_uid)
                .cloned()
                .ok_or_else(|| {
                    anyhow!(
                        "linked module `{provider_uid}` for import `{import_name}` of `{module_uid}` not found"
                    )
                })?;

            if provider.fuel != loaded.fuel {
                return Err(anyhow!(
                    "linked module `{provider_uid}` for import `{import_name}` of `{module_uid}` uses a different engine configuration"
                ));
            }

            if matches!(provider.mode, ModuleMode::Interpreter(_)) {
                return Err(anyhow!(
                    "linked module `{provider_uid}` for import `{import_name}` of `{module_uid}` uses interpreter mode, which is not supported for component links"
                ));
            }

            let provider_instance =
                instantiate_linked_module_inner(store, provider_uid, &provider, graph).await?;

            register_link_binding(
                linker,
                import_name,
                import_item,
                provider_uid,
                &provider.component,
                provider_instance,
            )?;
        }

        Ok(())
    })
}

fn register_link_binding(
    linker: &mut Linker<SyncWasmStates>,
    import_name: &str,
    import_item: ComponentItem,
    provider_uid: &str,
    provider_component: &Component,
    provider_instance: Instance,
) -> Result<()> {
    let engine = provider_component.engine();
    let provider_item = provider_component
        .component_type()
        .get_export(engine, import_name)
        .ok_or_else(|| {
            anyhow!(
                "linked module `{provider_uid}` does not export `{import_name}` required by the consumer"
            )
        })?;

    match (import_item, provider_item) {
        (
            ComponentItem::ComponentFunc(import_func),
            ComponentItem::ComponentFunc(provider_func),
        ) => {
            ensure_matching_function(
                import_name,
                import_name,
                provider_uid,
                &import_func,
                &provider_func,
            )?;
            let export_index = provider_component
                .get_export_index(None, import_name)
                .ok_or_else(|| {
                    anyhow!("failed to resolve export `{import_name}` in `{provider_uid}`")
                })?;
            linker.root().func_new_async(
                import_name,
                make_linked_func(
                    provider_uid.to_string(),
                    import_name.to_string(),
                    provider_instance,
                    export_index,
                ),
            )?;
            Ok(())
        }
        (
            ComponentItem::ComponentInstance(import_instance),
            ComponentItem::ComponentInstance(provider_instance_ty),
        ) => {
            let interface_index = provider_component
                .get_export_index(None, import_name)
                .ok_or_else(|| {
                    anyhow!("failed to resolve export `{import_name}` in `{provider_uid}`")
                })?;

            let mut instance = linker.instance(import_name)?;

            for (member_name, member_item) in import_instance.exports(engine) {
                match member_item {
                    ComponentItem::ComponentFunc(import_func) => {
                        let provider_member = provider_instance_ty
                            .get_export(engine, member_name)
                            .ok_or_else(|| {
                                anyhow!(
                                    "linked module `{provider_uid}` does not export function `{member_name}` in interface `{import_name}`"
                                )
                            })?;

                        let provider_func = match provider_member {
                            ComponentItem::ComponentFunc(func) => func,
                            other => {
                                return Err(anyhow!(
                                    "linked export `{import_name}/{member_name}` in `{provider_uid}` has unsupported item kind `{}`",
                                    component_item_kind(&other)
                                ));
                            }
                        };

                        ensure_matching_function(
                            import_name,
                            member_name,
                            provider_uid,
                            &import_func,
                            &provider_func,
                        )?;

                        let export_index = provider_component
                            .get_export_index(Some(&interface_index), member_name)
                            .ok_or_else(|| {
                                anyhow!(
                                    "failed to resolve export `{import_name}/{member_name}` in `{provider_uid}`"
                                )
                            })?;

                        instance.func_new_async(
                            member_name,
                            make_linked_func(
                                provider_uid.to_string(),
                                format!("{import_name}/{member_name}"),
                                provider_instance,
                                export_index,
                            ),
                        )?;
                    }
                    ComponentItem::Type(_) => {}
                    other => {
                        return Err(anyhow!(
                            "import `{import_name}` contains unsupported item kind `{}`; component links currently support function-based interfaces only",
                            component_item_kind(&other)
                        ));
                    }
                }
            }

            Ok(())
        }
        (expected, actual) => Err(anyhow!(
            "linked export `{import_name}` in `{provider_uid}` has kind `{}`, expected `{}`",
            component_item_kind(&actual),
            component_item_kind(&expected)
        )),
    }
}

fn make_linked_func(
    provider_uid: String,
    export_path: String,
    provider_instance: Instance,
    export_index: ComponentExportIndex,
) -> Box<LinkedFunc> {
    Box::new(move |mut store, _ty, params, results| {
        let provider_uid = provider_uid.clone();
        let export_path = export_path.clone();
        Box::new(async move {
            let func = provider_instance
                .get_func(&mut store, export_index)
                .ok_or_else(|| {
                    anyhow!(
                        "linked export `{export_path}` in `{provider_uid}` is not a function at runtime"
                    )
                })?;

            func.call_async(&mut store, params, results).await?;
            func.post_return_async(&mut store).await?;
            Ok(())
        })
    })
}

fn ensure_matching_function(
    import_name: &str,
    member_name: &str,
    provider_uid: &str,
    import_func: &wasmtime::component::types::ComponentFunc,
    provider_func: &wasmtime::component::types::ComponentFunc,
) -> Result<()> {
    if component_funcs_match(import_func, provider_func) {
        return Ok(());
    }

    let member_path = if import_name == member_name {
        import_name.to_string()
    } else {
        format!("{import_name}/{member_name}")
    };

    Err(anyhow!(
        "linked export `{member_path}` in `{provider_uid}` does not match the consumer import type"
    ))
}

fn component_funcs_match(
    lhs: &wasmtime::component::types::ComponentFunc,
    rhs: &wasmtime::component::types::ComponentFunc,
) -> bool {
    if lhs.async_() != rhs.async_() {
        return false;
    }

    let lhs_params = lhs.params().map(|(_, ty)| ty).collect::<Vec<_>>();
    let rhs_params = rhs.params().map(|(_, ty)| ty).collect::<Vec<_>>();
    if lhs_params != rhs_params {
        return false;
    }

    let lhs_results = lhs.results().collect::<Vec<_>>();
    let rhs_results = rhs.results().collect::<Vec<_>>();
    lhs_results == rhs_results
}

fn component_item_kind(item: &ComponentItem) -> &'static str {
    match item {
        ComponentItem::ComponentFunc(_) => "function",
        ComponentItem::CoreFunc(_) => "core-function",
        ComponentItem::Module(_) => "module",
        ComponentItem::Component(_) => "component",
        ComponentItem::ComponentInstance(_) => "instance",
        ComponentItem::Type(_) => "type",
        ComponentItem::Resource(_) => "resource",
    }
}

pub(super) fn command_run_job(component: &Component) -> Result<CallJob> {
    let engine = component.engine();
    let ty = component.component_type();

    for (name, item) in ty.exports(engine) {
        if !name.starts_with("wasi:cli/run") {
            continue;
        }

        if let ComponentItem::ComponentInstance(instance) = item {
            if matches!(
                instance.get_export(engine, "run"),
                Some(ComponentItem::ComponentFunc(_))
            ) {
                return Ok(CallJob::new(
                    Some(name.to_string()),
                    "run".to_string(),
                    Vec::new(),
                ));
            }
        }
    }

    Err(anyhow!("component does not export `wasi:cli/run`"))
}
