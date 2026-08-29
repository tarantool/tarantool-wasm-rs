use std::sync::RwLock;
use std::time::Instant;

use anyhow::{anyhow, Result};
use hashbrown::{Equivalent, HashMap};
use rmpv::Value as RmpvValue;
use wasmtime::{
    component::{Component, ComponentExportIndex, Instance, InstancePre, Val},
    AsContext, Store,
};

use crate::runtime::convert::{to_component_val, val_to_rmpv};
use crate::runtime::metrics;

use super::{
    resources::{from_session_val, to_session_val, SessionCallResult},
    SessionResources,
};
use super::{CallJob, ExportCacheKey, SyncWasmStates};

#[derive(Hash)]
struct ExportCacheKeyRef<'a> {
    interface: Option<&'a str>,
    func: &'a str,
}

impl Equivalent<ExportCacheKey> for ExportCacheKeyRef<'_> {
    fn equivalent(&self, key: &ExportCacheKey) -> bool {
        self.interface == key.interface.as_deref() && self.func == key.func
    }
}

pub(super) type ExportCache = RwLock<HashMap<ExportCacheKey, ComponentExportIndex>>;

pub(super) fn resolve_call_export_index(
    component: &Component,
    export_cache: &ExportCache,
    job: &CallJob,
) -> Result<ComponentExportIndex> {
    let borrowed_key = ExportCacheKeyRef {
        interface: job.interface.as_deref(),
        func: &job.func,
    };
    if let Some(index) = export_cache.read().unwrap().get(&borrowed_key).copied() {
        metrics::observe_export_cache_hit();
        return Ok(index);
    }
    metrics::observe_export_cache_miss();

    let resolved = if let Some(interface_name) = job.interface.as_deref() {
        let interface_index = component
            .get_export_index(None, interface_name)
            .ok_or_else(|| anyhow!("interface export `{}` not found", interface_name))?;

        component
            .get_export_index(Some(&interface_index), &job.func)
            .ok_or_else(|| {
                anyhow!(
                    "function `{}` not found in interface `{}`",
                    job.func,
                    interface_name
                )
            })?
    } else {
        component
            .get_export_index(None, &job.func)
            .ok_or_else(|| anyhow!("function `{}` not found at top level", job.func))?
    };

    let key = job.cache_key();
    let mut cache = export_cache.write().unwrap();
    Ok(*cache.entry(key).or_insert(resolved))
}

fn values_to_rmpv(results_buf: &[Val]) -> Result<RmpvValue> {
    match results_buf {
        [] => Ok(RmpvValue::Nil),
        [val] => val_to_rmpv(val),
        many => {
            let converted = many.iter().map(val_to_rmpv).collect::<Result<Vec<_>>>()?;
            Ok(RmpvValue::Array(converted))
        }
    }
}

pub(super) async fn execute_call_job(
    store: &mut Store<SyncWasmStates>,
    instance_pre: &InstancePre<SyncWasmStates>,
    component: &Component,
    export_cache: &ExportCache,
    job: &CallJob,
) -> Result<RmpvValue> {
    let started = Instant::now();
    let instance_result = instance_pre.instantiate_async(&mut *store).await;
    metrics::observe_instantiation(started.elapsed());
    let instance = instance_result?;
    execute_call_job_on_instance(store, &instance, component, export_cache, job).await
}

pub(super) async fn execute_call_job_on_instance(
    store: &mut Store<SyncWasmStates>,
    instance: &Instance,
    component: &Component,
    export_cache: &ExportCache,
    job: &CallJob,
) -> Result<RmpvValue> {
    let state = store.data_mut();
    state.set_caller_session_id(job.caller_session_id);
    state.enter_call(job.call_chain.clone())?;
    let func_index = resolve_call_export_index(component, export_cache, job)?;

    let export = instance
        .get_func(&mut *store, func_index)
        .ok_or_else(|| anyhow!("function export `{}` not found", job.func))?;

    let export_ty = export.ty(store.as_context());
    let param_types = export_ty.params().map(|(_, t)| t).collect::<Vec<_>>();

    if job.args.len() != param_types.len() {
        return Err(anyhow!(
            "argument count mismatch for `{}`: expected {}, got {}",
            job.func,
            param_types.len(),
            job.args.len()
        ));
    }

    let lowering_started = Instant::now();
    let mut wasm_args = Vec::with_capacity(param_types.len());
    let lowering_result = (|| {
        for (ty, val) in param_types.iter().zip(job.args.iter()) {
            match to_component_val(ty, val) {
                Ok(value) => wasm_args.push(value),
                Err(err) => {
                    metrics::observe_conversion_error("arguments");
                    return Err(err);
                }
            }
        }
        Ok(())
    })();
    metrics::observe_argument_lowering(lowering_started.elapsed());
    lowering_result?;

    let result_types = export_ty.results().collect::<Vec<_>>();
    let mut results_buf = vec![Val::Bool(false); result_types.len()];

    let fuel_before = store.get_fuel().ok();
    let started = Instant::now();
    let call_result = export
        .call_async(&mut *store, &wasm_args, &mut results_buf)
        .await;
    metrics::observe_guest_execution(started.elapsed());
    metrics::observe_fuel(fuel_before, store.get_fuel().ok());
    if let Err(err) = &call_result {
        metrics::observe_trap(err);
    }
    call_result?;

    let lifting_started = Instant::now();
    let result = values_to_rmpv(&results_buf);
    metrics::observe_result_lifting(lifting_started.elapsed());
    result.inspect_err(|_| {
        metrics::observe_conversion_error("results");
    })
}

pub(super) async fn execute_session_call_job_on_instance(
    store: &mut Store<SyncWasmStates>,
    instance: &Instance,
    component: &Component,
    export_cache: &ExportCache,
    resources: &mut SessionResources,
    job: &CallJob,
) -> Result<SessionCallResult> {
    let state = store.data_mut();
    state.set_caller_session_id(job.caller_session_id);
    state.enter_call(job.call_chain.clone())?;
    let func_index = resolve_call_export_index(component, export_cache, job)?;
    let export = instance
        .get_func(&mut *store, func_index)
        .ok_or_else(|| anyhow!("function export `{}` not found", job.func))?;
    let export_ty = export.ty(store.as_context());
    let param_types = export_ty.params().map(|(_, ty)| ty).collect::<Vec<_>>();
    if job.args.len() != param_types.len() {
        return Err(anyhow!(
            "argument count mismatch for `{}`: expected {}, got {}",
            job.func,
            param_types.len(),
            job.args.len()
        ));
    }

    let mut consumed = Vec::new();
    let lowering_started = Instant::now();
    let wasm_args = param_types
        .iter()
        .zip(job.args.iter())
        .map(|(ty, value)| to_session_val(ty, value, resources, &mut consumed))
        .collect::<Result<Vec<_>>>();
    metrics::observe_argument_lowering(lowering_started.elapsed());
    let wasm_args = match wasm_args {
        Ok(args) => args,
        Err(err) => {
            metrics::observe_conversion_error("resource_arguments");
            resources.restore(consumed);
            return Err(err);
        }
    };
    let consumed = consumed.iter().map(|(id, _)| *id).collect();

    let result_types = export_ty.results().collect::<Vec<_>>();
    let mut results = vec![Val::Bool(false); result_types.len()];
    let fuel_before = store.get_fuel().ok();
    let started = Instant::now();
    let call_result = export
        .call_async(&mut *store, &wasm_args, &mut results)
        .await;
    metrics::observe_guest_execution(started.elapsed());
    metrics::observe_fuel(fuel_before, store.get_fuel().ok());
    if let Err(err) = &call_result {
        metrics::observe_trap(err);
    }
    call_result?;

    let lifting_started = Instant::now();
    let value = match results.as_slice() {
        [] => Ok(RmpvValue::Nil),
        [value] => from_session_val(&result_types[0], value, resources),
        values => values
            .iter()
            .zip(result_types)
            .map(|(value, ty)| from_session_val(&ty, value, resources))
            .collect::<Result<_>>()
            .map(RmpvValue::Array),
    };
    metrics::observe_result_lifting(lifting_started.elapsed());
    let value = value.inspect_err(|_| metrics::observe_conversion_error("resource_results"))?;
    Ok(SessionCallResult { value, consumed })
}

#[cfg(test)]
mod tests {
    use super::{ExportCacheKey, ExportCacheKeyRef};
    use hashbrown::HashMap;

    #[test]
    fn borrowed_export_keys_match_owned_cache_entries() {
        let mut cache = HashMap::new();
        cache.insert(
            ExportCacheKey {
                interface: None,
                func: "echo".to_string(),
            },
            1_u32,
        );
        cache.insert(
            ExportCacheKey {
                interface: Some("example:first/api@1.0.0".to_string()),
                func: "echo".to_string(),
            },
            2_u32,
        );
        cache.insert(
            ExportCacheKey {
                interface: Some("example:second/api@1.0.0".to_string()),
                func: "echo".to_string(),
            },
            3_u32,
        );

        assert_eq!(
            cache.get(&ExportCacheKeyRef {
                interface: None,
                func: "echo",
            }),
            Some(&1)
        );
        assert_eq!(
            cache.get(&ExportCacheKeyRef {
                interface: Some("example:first/api@1.0.0"),
                func: "echo",
            }),
            Some(&2)
        );
        assert_eq!(
            cache.get(&ExportCacheKeyRef {
                interface: Some("example:second/api@1.0.0"),
                func: "echo",
            }),
            Some(&3)
        );
        assert_eq!(
            cache.get(&ExportCacheKeyRef {
                interface: Some("example:missing/api@1.0.0"),
                func: "echo",
            }),
            None
        );
    }
}
