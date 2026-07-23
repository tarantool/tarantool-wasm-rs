use super::*;

use wasmtime::component::{
    types::{ComponentInstance, ComponentItem},
    ResourceAny, ResourceType, Type, Val,
};

const RESOURCE_EXT_TAG: i8 = 42;

#[derive(Debug)]
pub(crate) struct ResourceHandle {
    pub(crate) id: u64,
    pub(crate) owned: bool,
    pub(crate) interface: Option<String>,
    pub(crate) resource: String,
}

pub(crate) struct SessionCallResult {
    pub(crate) value: RmpvValue,
    pub(crate) consumed: Vec<u64>,
}

#[derive(Clone)]
struct ResourceDescriptor {
    ty: ResourceType,
    interface: Option<String>,
    name: String,
}

pub(super) struct SessionResources {
    next_id: u64,
    values: HashMap<u64, ResourceAny>,
    descriptors: Vec<ResourceDescriptor>,
}

impl SessionResources {
    pub(super) fn new(
        engine: &Engine,
        component: &Component,
        instance: &Instance,
        store: &mut Store<SyncWasmStates>,
    ) -> Self {
        Self::new_with_next_id(engine, component, instance, store, 1)
    }

    pub(super) fn new_with_next_id(
        engine: &Engine,
        component: &Component,
        instance: &Instance,
        store: &mut Store<SyncWasmStates>,
        next_id: u64,
    ) -> Self {
        let mut descriptors = Vec::new();
        let component_ty = component.component_type();
        let mut discovery = ResourceDiscovery {
            engine,
            component,
            instance,
            store,
            out: &mut descriptors,
        };
        for (name, item) in component_ty.exports(engine) {
            let index = component.get_export_index(None, name);
            discovery.collect_item(item.ty, index, None, name);
        }
        Self {
            next_id,
            values: HashMap::new(),
            descriptors,
        }
    }

    pub(super) fn next_id(&self) -> u64 {
        self.next_id
    }

    fn lower(
        &mut self,
        expected: &Type,
        value: &RmpvValue,
        consumed: &mut Vec<(u64, ResourceAny)>,
    ) -> Result<Val> {
        let handle = decode_resource_handle(value)?;
        let resource = match expected {
            Type::Own(_) => {
                let resource = self
                    .values
                    .remove(&handle.id)
                    .ok_or_else(|| anyhow!("resource #{} is no longer available", handle.id))?;
                consumed.push((handle.id, resource));
                resource
            }
            Type::Borrow(_) => *self
                .values
                .get(&handle.id)
                .ok_or_else(|| anyhow!("resource #{} is no longer available", handle.id))?,
            _ => unreachable!(),
        };
        Ok(Val::Resource(resource))
    }

    fn lift(&mut self, resource: ResourceAny) -> RmpvValue {
        let id = self.next_id;
        self.next_id += 1;
        let descriptor = self
            .descriptors
            .iter()
            .find(|descriptor| descriptor.ty == resource.ty());
        let (interface, name) = match descriptor {
            Some(descriptor) => (descriptor.interface.clone(), descriptor.name.clone()),
            None => (None, "resource".to_string()),
        };
        self.values.insert(id, resource);
        encode_resource_handle(id, resource.owned(), interface.as_deref(), &name)
    }

    pub(super) fn restore(&mut self, consumed: Vec<(u64, ResourceAny)>) {
        self.values.extend(consumed);
    }

    pub(super) async fn drop_resource(
        &mut self,
        store: &mut Store<SyncWasmStates>,
        id: u64,
    ) -> Result<()> {
        let resource = self
            .values
            .remove(&id)
            .ok_or_else(|| anyhow!("resource #{id} is no longer available"))?;
        resource
            .resource_drop_async(store)
            .await
            .map_err(Into::into)
    }

    pub(super) async fn drop_all(&mut self, store: &mut Store<SyncWasmStates>) -> Result<()> {
        let resources = self.values.drain().map(|(_, resource)| resource);
        let mut first_error = None;
        for resource in resources {
            if let Err(err) = resource.resource_drop_async(&mut *store).await {
                if first_error.is_none() {
                    first_error = Some(err);
                }
            }
        }
        match first_error {
            Some(err) => Err(err.into()),
            None => Ok(()),
        }
    }
}

struct ResourceDiscovery<'a> {
    engine: &'a Engine,
    component: &'a Component,
    instance: &'a Instance,
    store: &'a mut Store<SyncWasmStates>,
    out: &'a mut Vec<ResourceDescriptor>,
}

impl ResourceDiscovery<'_> {
    fn collect_item(
        &mut self,
        item: ComponentItem,
        index: Option<ComponentExportIndex>,
        namespace: Option<&str>,
        name: &str,
    ) {
        match item {
            ComponentItem::Resource(_) => {
                if let Some(ty) =
                    index.and_then(|index| self.instance.get_resource(&mut *self.store, index))
                {
                    self.out.push(ResourceDescriptor {
                        ty,
                        interface: namespace.map(str::to_string),
                        name: name.to_string(),
                    });
                }
            }
            ComponentItem::ComponentInstance(nested) => {
                let Some(parent) = index else {
                    return;
                };
                let nested_namespace = match namespace {
                    Some(namespace) => format!("{namespace}/{name}"),
                    None => name.to_string(),
                };
                self.collect_instance(&nested, parent, &nested_namespace);
            }
            _ => {}
        }
    }

    fn collect_instance(
        &mut self,
        instance_ty: &ComponentInstance,
        parent: ComponentExportIndex,
        namespace: &str,
    ) {
        for (name, item) in instance_ty.exports(self.engine) {
            let index = self.component.get_export_index(Some(&parent), name);
            match item.ty {
                ComponentItem::Resource(_) => {
                    if let Some(ty) =
                        index.and_then(|index| self.instance.get_resource(&mut *self.store, index))
                    {
                        self.out.push(ResourceDescriptor {
                            ty,
                            interface: Some(namespace.to_string()),
                            name: name.to_string(),
                        });
                    }
                }
                ComponentItem::ComponentInstance(nested) => {
                    let Some(parent) = index else {
                        continue;
                    };
                    self.collect_instance(&nested, parent, &format!("{namespace}/{name}"));
                }
                _ => {}
            }
        }
    }
}

pub(super) fn to_session_val(
    ty: &Type,
    value: &RmpvValue,
    resources: &mut SessionResources,
    consumed: &mut Vec<(u64, ResourceAny)>,
) -> Result<Val> {
    Ok(match ty {
        Type::Own(_) | Type::Borrow(_) => resources.lower(ty, value, consumed)?,
        Type::List(list) => Val::List(
            value
                .as_array()
                .ok_or_else(|| anyhow!("expected array for list"))?
                .iter()
                .map(|value| to_session_val(&list.ty(), value, resources, consumed))
                .collect::<Result<_>>()?,
        ),
        Type::Map(map) => Val::Map(
            value
                .as_map()
                .ok_or_else(|| anyhow!("expected map"))?
                .iter()
                .map(|(key, value)| {
                    Ok((
                        to_session_val(&map.key(), key, resources, consumed)?,
                        to_session_val(&map.value(), value, resources, consumed)?,
                    ))
                })
                .collect::<Result<_>>()?,
        ),
        Type::Option(option) => match value {
            RmpvValue::Nil => Val::Option(None),
            value => Val::Option(Some(Box::new(to_session_val(
                &option.ty(),
                value,
                resources,
                consumed,
            )?))),
        },
        Type::Tuple(tuple) => {
            let values = value
                .as_array()
                .ok_or_else(|| anyhow!("expected array for tuple"))?;
            let types = tuple.types().collect::<Vec<_>>();
            if values.len() != types.len() {
                return Err(anyhow!(
                    "tuple length mismatch: expected {}, got {}",
                    types.len(),
                    values.len()
                ));
            }
            Val::Tuple(
                values
                    .iter()
                    .zip(types)
                    .map(|(value, ty)| to_session_val(&ty, value, resources, consumed))
                    .collect::<Result<_>>()?,
            )
        }
        Type::Record(record) => {
            let values = value
                .as_map()
                .ok_or_else(|| anyhow!("expected map for record"))?;
            let values = values
                .iter()
                .filter_map(|(key, value)| key.as_str().map(|key| (key, value)))
                .collect::<HashMap<_, _>>();
            let mut fields = Vec::new();
            for field in record.fields() {
                let value = match values.get(field.name) {
                    Some(value) => *value,
                    None if matches!(field.ty, Type::Option(_)) => &RmpvValue::Nil,
                    None => return Err(anyhow!("missing field `{}` in record", field.name)),
                };
                fields.push((
                    field.name.to_string(),
                    to_session_val(&field.ty, value, resources, consumed)?,
                ));
            }
            Val::Record(fields)
        }
        Type::Variant(variant) => {
            let values = value
                .as_map()
                .ok_or_else(|| anyhow!("expected map for variant"))?;
            if values.len() != 1 {
                return Err(anyhow!("expected exactly one key in variant"));
            }
            let (name, value) = &values[0];
            let name = name
                .as_str()
                .ok_or_else(|| anyhow!("variant key must be string"))?;
            let case = variant
                .cases()
                .find(|case| case.name == name)
                .ok_or_else(|| anyhow!("unknown variant case: {name}"))?;
            let payload = case
                .ty
                .map(|ty| to_session_val(&ty, value, resources, consumed).map(Box::new))
                .transpose()?;
            Val::Variant(name.to_string(), payload)
        }
        Type::Result(result) => {
            let values = value
                .as_array()
                .ok_or_else(|| anyhow!("expected [ok, value] for result"))?;
            if values.len() != 2 {
                return Err(anyhow!("expected [ok, value] for result"));
            }
            let ok = values[0]
                .as_bool()
                .ok_or_else(|| anyhow!("result status must be boolean"))?;
            let ty = if ok { result.ok() } else { result.err() };
            let payload = ty
                .map(|ty| {
                    to_session_val(&ty, &values[1], resources, consumed)
                        .map(Box::new)
                        .map(Some)
                })
                .unwrap_or(Ok(None))?;
            Val::Result(if ok { Ok(payload) } else { Err(payload) })
        }
        _ => crate::runtime::convert::to_component_val(ty, value)?,
    })
}

pub(super) fn from_session_val(
    ty: &Type,
    value: &Val,
    resources: &mut SessionResources,
) -> Result<RmpvValue> {
    Ok(match (ty, value) {
        (Type::Own(_), Val::Resource(resource)) | (Type::Borrow(_), Val::Resource(resource)) => {
            resources.lift(*resource)
        }
        (Type::List(list), Val::List(values)) => RmpvValue::Array(
            values
                .iter()
                .map(|value| from_session_val(&list.ty(), value, resources))
                .collect::<Result<_>>()?,
        ),
        (Type::Map(map), Val::Map(values)) => RmpvValue::Map(
            values
                .iter()
                .map(|(key, value)| {
                    Ok((
                        from_session_val(&map.key(), key, resources)?,
                        from_session_val(&map.value(), value, resources)?,
                    ))
                })
                .collect::<Result<_>>()?,
        ),
        (Type::Option(_), Val::Option(None)) => RmpvValue::Nil,
        (Type::Option(option), Val::Option(Some(value))) => {
            from_session_val(&option.ty(), value, resources)?
        }
        (Type::Tuple(tuple), Val::Tuple(values)) => {
            let types = tuple.types().collect::<Vec<_>>();
            if values.len() != types.len() {
                return Err(anyhow!("component returned tuple with invalid length"));
            }
            RmpvValue::Array(
                values
                    .iter()
                    .zip(types)
                    .map(|(value, ty)| from_session_val(&ty, value, resources))
                    .collect::<Result<_>>()?,
            )
        }
        (Type::Record(record), Val::Record(values)) => {
            let types = record.fields().collect::<Vec<_>>();
            if values.len() != types.len() {
                return Err(anyhow!("component returned record with invalid length"));
            }
            RmpvValue::Map(
                values
                    .iter()
                    .zip(types)
                    .map(|((name, value), field)| {
                        Ok((
                            RmpvValue::String(name.clone().into()),
                            from_session_val(&field.ty, value, resources)?,
                        ))
                    })
                    .collect::<Result<_>>()?,
            )
        }
        (Type::Variant(variant), Val::Variant(name, value)) => {
            let case = variant
                .cases()
                .find(|case| case.name == name)
                .ok_or_else(|| anyhow!("component returned unknown variant case `{name}`"))?;
            let value = match (&case.ty, value) {
                (Some(ty), Some(value)) => from_session_val(ty, value, resources)?,
                (None, None) => RmpvValue::Nil,
                _ => return Err(anyhow!("component returned invalid variant payload")),
            };
            RmpvValue::Map(vec![(RmpvValue::String(name.clone().into()), value)])
        }
        (Type::Result(result), Val::Result(value)) => {
            let (ok, ty, value) = match value {
                Ok(value) => (true, result.ok(), value),
                Err(value) => (false, result.err(), value),
            };
            let payload = match (ty, value) {
                (Some(ty), Some(value)) => from_session_val(&ty, value, resources)?,
                (None, None) => RmpvValue::Nil,
                _ => return Err(anyhow!("component returned invalid result payload")),
            };
            RmpvValue::Array(vec![RmpvValue::Boolean(ok), payload])
        }
        _ => crate::runtime::convert::val_to_rmpv(value)?,
    })
}

fn encode_resource_handle(
    id: u64,
    owned: bool,
    interface: Option<&str>,
    resource: &str,
) -> RmpvValue {
    let interface = interface.unwrap_or_default().as_bytes();
    let resource = resource.as_bytes();
    let mut bytes = Vec::with_capacity(13 + interface.len() + resource.len());
    bytes.extend_from_slice(&id.to_le_bytes());
    bytes.push(u8::from(owned));
    bytes.extend_from_slice(&(interface.len() as u16).to_le_bytes());
    bytes.extend_from_slice(interface);
    bytes.extend_from_slice(&(resource.len() as u16).to_le_bytes());
    bytes.extend_from_slice(resource);
    RmpvValue::Ext(RESOURCE_EXT_TAG, bytes)
}

pub(crate) fn encode_resource_argument(id: u64) -> RmpvValue {
    encode_resource_handle(id, true, None, "")
}

pub(crate) fn decode_resource_handle(value: &RmpvValue) -> Result<ResourceHandle> {
    let RmpvValue::Ext(tag, bytes) = value else {
        return Err(anyhow!("expected WIT resource"));
    };
    if *tag != RESOURCE_EXT_TAG || bytes.len() < 13 {
        return Err(anyhow!("expected WIT resource"));
    }
    let id = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
    let owned = bytes[8] != 0;
    let interface_len = u16::from_le_bytes(bytes[9..11].try_into().unwrap()) as usize;
    let interface_end = 11 + interface_len;
    if bytes.len() < interface_end + 2 {
        return Err(anyhow!("invalid WIT resource handle"));
    }
    let resource_len =
        u16::from_le_bytes(bytes[interface_end..interface_end + 2].try_into().unwrap()) as usize;
    let resource_end = interface_end + 2 + resource_len;
    if bytes.len() != resource_end {
        return Err(anyhow!("invalid WIT resource handle"));
    }
    let interface = std::str::from_utf8(&bytes[11..interface_end])?;
    let resource = std::str::from_utf8(&bytes[interface_end + 2..resource_end])?;
    Ok(ResourceHandle {
        id,
        owned,
        interface: (!interface.is_empty()).then(|| interface.to_string()),
        resource: resource.to_string(),
    })
}
