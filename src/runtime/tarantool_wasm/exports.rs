use wasmtime::component::types::{ComponentFunc, ComponentInstance, ComponentItem};

use crate::runtime::convert::pretty_type;

use super::{ExportInfo, ExportMap};

pub(super) fn export_info_from_component_func(func: ComponentFunc) -> ExportInfo {
    let params = func
        .params()
        .map(|(name, ty)| (name.to_string(), pretty_type(&ty)))
        .collect();
    let results = func.results().map(|ty| pretty_type(&ty)).collect();
    ExportInfo { params, results }
}

pub(super) fn collect_exports(
    engine: &wasmtime::Engine,
    instance: &ComponentInstance,
    namespace: &str,
    out: &mut ExportMap,
) {
    for (name, item) in instance.exports(engine) {
        match item {
            ComponentItem::ComponentFunc(func) => {
                out.entry(namespace.to_string())
                    .or_default()
                    .insert(name.to_string(), export_info_from_component_func(func));
            }
            ComponentItem::ComponentInstance(subinst) => {
                let nested_namespace = format!("{namespace}/{name}");
                collect_exports(engine, &subinst, &nested_namespace, out);
            }
            _ => {}
        }
    }
}
