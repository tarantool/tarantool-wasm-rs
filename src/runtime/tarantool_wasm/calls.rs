use std::{collections::HashMap, sync::RwLock};

use anyhow::{anyhow, Result};
use rmpv::Value as RmpvValue;
use wasmtime::{
    component::{Component, ComponentExportIndex, Instance, InstancePre, Val},
    AsContext, Store,
};

use crate::runtime::convert::{to_component_val, val_to_rmpv};

use super::{
    resources::{from_session_val, to_session_val, SessionCallResult},
    SessionResources,
};
use super::{CallJob, ExportCacheKey, SyncWasmStates};

fn resolve_call_export_index(
    component: &Component,
    export_cache: &RwLock<HashMap<ExportCacheKey, ComponentExportIndex>>,
    job: &CallJob,
) -> Result<ComponentExportIndex> {
    let key = job.cache_key();

    if let Some(index) = export_cache.read().unwrap().get(&key).copied() {
        return Ok(index);
    }

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
    export_cache: &RwLock<HashMap<ExportCacheKey, ComponentExportIndex>>,
    job: &CallJob,
) -> Result<RmpvValue> {
    let instance = instance_pre.instantiate_async(&mut *store).await?;
    execute_call_job_on_instance(store, &instance, component, export_cache, job).await
}

pub(super) async fn execute_call_job_on_instance(
    store: &mut Store<SyncWasmStates>,
    instance: &Instance,
    component: &Component,
    export_cache: &RwLock<HashMap<ExportCacheKey, ComponentExportIndex>>,
    job: &CallJob,
) -> Result<RmpvValue> {
    store
        .data_mut()
        .set_caller_session_id(job.caller_session_id);
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

    let mut wasm_args = Vec::with_capacity(param_types.len());
    for (ty, val) in param_types.iter().zip(job.args.iter()) {
        wasm_args.push(to_component_val(ty, val)?);
    }

    let result_types = export_ty.results().collect::<Vec<_>>();
    let mut results_buf = vec![Val::Bool(false); result_types.len()];

    export
        .call_async(&mut *store, &wasm_args, &mut results_buf)
        .await?;

    values_to_rmpv(&results_buf)
}

pub(super) async fn execute_session_call_job_on_instance(
    store: &mut Store<SyncWasmStates>,
    instance: &Instance,
    component: &Component,
    export_cache: &RwLock<HashMap<ExportCacheKey, ComponentExportIndex>>,
    resources: &mut SessionResources,
    job: &CallJob,
) -> Result<SessionCallResult> {
    store
        .data_mut()
        .set_caller_session_id(job.caller_session_id);
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
    let wasm_args = match param_types
        .iter()
        .zip(job.args.iter())
        .map(|(ty, value)| to_session_val(ty, value, resources, &mut consumed))
        .collect::<Result<Vec<_>>>()
    {
        Ok(args) => args,
        Err(err) => {
            resources.restore(consumed);
            return Err(err);
        }
    };
    let consumed = consumed.iter().map(|(id, _)| *id).collect();

    let result_types = export_ty.results().collect::<Vec<_>>();
    let mut results = vec![Val::Bool(false); result_types.len()];
    export
        .call_async(&mut *store, &wasm_args, &mut results)
        .await?;

    let value = match results.as_slice() {
        [] => Ok(RmpvValue::Nil),
        [value] => from_session_val(&result_types[0], value, resources),
        values => Ok(RmpvValue::Array(
            values
                .iter()
                .zip(result_types)
                .map(|(value, ty)| from_session_val(&ty, value, resources))
                .collect::<Result<_>>()?,
        )),
    }?;
    Ok(SessionCallResult { value, consumed })
}
