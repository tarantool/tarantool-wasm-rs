use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use mlua::prelude::*;
use rmpv::Value as RmpvValue;

use crate::runtime::{
    convert::{from_rmpv, to_rmpv},
    tarantool_wasm::{
        resources::{decode_resource_handle, encode_resource_argument},
        tt_wasm_close_session_detached, tt_wasm_session_call_function_with_ownership,
        tt_wasm_session_drop_resource, tt_wasm_session_drop_resource_detached,
    },
};

pub(crate) struct SessionLease {
    handle: String,
    close_on_drop: bool,
}

impl SessionLease {
    pub(crate) fn owned(handle: String) -> Arc<Self> {
        Arc::new(Self {
            handle,
            close_on_drop: true,
        })
    }

    pub(crate) fn borrowed(handle: String) -> Arc<Self> {
        Arc::new(Self {
            handle,
            close_on_drop: false,
        })
    }

    pub(crate) fn handle(&self) -> &str {
        &self.handle
    }
}

impl Drop for SessionLease {
    fn drop(&mut self) {
        if self.close_on_drop {
            tt_wasm_close_session_detached(&self.handle);
        }
    }
}

struct ResourceState {
    session: Arc<SessionLease>,
    id: u64,
    owned: bool,
    interface: Option<String>,
    resource: String,
    alive: AtomicBool,
}

impl Drop for ResourceState {
    fn drop(&mut self) {
        if self.alive.swap(false, Ordering::AcqRel) {
            tt_wasm_session_drop_resource_detached(self.session.handle(), self.id);
        }
    }
}

#[derive(Clone)]
pub(crate) struct LuaWasmResource(Arc<ResourceState>);

impl LuaWasmResource {
    fn drop_explicit(&self) -> LuaResult<bool> {
        if self
            .0
            .alive
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(false);
        }
        if let Err(err) =
            tt_wasm_session_drop_resource(self.0.session.handle().to_string(), self.0.id)
        {
            self.0.alive.store(true, Ordering::Release);
            return Err(mlua::Error::external(err));
        }
        Ok(true)
    }

    fn encode(&self, session: &SessionLease) -> LuaResult<RmpvValue> {
        if self.0.session.handle() != session.handle() {
            return Err(LuaError::runtime(
                "WIT resource belongs to a different WASM session",
            ));
        }
        if !self.0.alive.load(Ordering::Acquire) {
            return Err(LuaError::runtime("WIT resource has been dropped"));
        }
        Ok(encode_resource_argument(self.0.id))
    }

    fn call_method(&self, lua: &Lua, method: &str, args: LuaMultiValue) -> LuaResult<LuaValue> {
        if !self.0.alive.load(Ordering::Acquire) {
            return Err(LuaError::runtime("WIT resource has been dropped"));
        }
        let mut iter = args.into_iter();
        let _self = iter.next();
        let mut values = vec![encode_resource_argument(self.0.id)];
        let mut used = Vec::new();
        for value in iter {
            values.push(to_rmpv_with_session_tracking(
                lua,
                &value,
                self.0.session.as_ref(),
                &mut used,
            )?);
        }
        let result = tt_wasm_session_call_function_with_ownership(
            self.0.session.handle().to_string(),
            self.0.interface.clone(),
            format!("[method]{}.{}", self.0.resource, method),
            values,
        )
        .map_err(mlua::Error::external)?;
        mark_consumed(&used, &result.consumed);
        from_rmpv_with_session(lua, &result.value, Arc::clone(&self.0.session))
    }
}

impl LuaUserData for LuaWasmResource {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("drop", |_, this, ()| this.drop_explicit());
        methods.add_method("owned", |_, this, ()| Ok(this.0.owned));
        methods.add_method("type", |_, this, ()| Ok(this.0.resource.clone()));
        methods.add_method("is_alive", |_, this, ()| {
            Ok(this.0.alive.load(Ordering::Acquire))
        });
        methods.add_meta_method(LuaMetaMethod::Index, |lua, this, method: String| {
            let resource = this.clone();
            lua.create_function(move |lua, args: LuaMultiValue| {
                resource.call_method(lua, &method, args)
            })
        });
        methods.add_meta_method(LuaMetaMethod::ToString, |_, this, ()| {
            let state = if this.0.alive.load(Ordering::Acquire) {
                "alive"
            } else {
                "dropped"
            };
            Ok(format!(
                "wasm.resource<{}>#{} ({state})",
                this.0.resource, this.0.id
            ))
        });
    }
}

pub(crate) fn to_rmpv_with_session_tracking(
    lua: &Lua,
    value: &LuaValue,
    session: &SessionLease,
    used: &mut Vec<LuaWasmResource>,
) -> LuaResult<RmpvValue> {
    Ok(match value {
        LuaValue::Table(table) => {
            let is_array = table.clone().pairs::<LuaValue, LuaValue>().all(|pair| {
                pair.map(|(key, _)| matches!(key, LuaValue::Integer(_)))
                    .unwrap_or(false)
            });
            if is_array {
                RmpvValue::Array(
                    table
                        .clone()
                        .sequence_values::<LuaValue>()
                        .map(|value| {
                            value.and_then(|value| {
                                to_rmpv_with_session_tracking(lua, &value, session, used)
                            })
                        })
                        .collect::<LuaResult<_>>()?,
                )
            } else {
                RmpvValue::Map(
                    table
                        .clone()
                        .pairs::<LuaValue, LuaValue>()
                        .map(|pair| {
                            let (key, value) = pair?;
                            Ok((
                                to_rmpv_with_session_tracking(lua, &key, session, used)?,
                                to_rmpv_with_session_tracking(lua, &value, session, used)?,
                            ))
                        })
                        .collect::<LuaResult<_>>()?,
                )
            }
        }
        LuaValue::UserData(userdata) if userdata.is::<LuaWasmResource>() => {
            let resource = userdata.borrow::<LuaWasmResource>()?.clone();
            let encoded = resource.encode(session)?;
            used.push(resource);
            encoded
        }
        _ => to_rmpv(lua, value)?,
    })
}

pub(crate) fn mark_consumed(resources: &[LuaWasmResource], consumed: &[u64]) {
    for resource in resources {
        if consumed.contains(&resource.0.id) {
            resource.0.alive.store(false, Ordering::Release);
        }
    }
}

pub(crate) fn from_rmpv_with_session(
    lua: &Lua,
    value: &RmpvValue,
    session: Arc<SessionLease>,
) -> LuaResult<LuaValue> {
    Ok(match value {
        RmpvValue::Array(values) => LuaValue::Table(
            lua.create_sequence_from(
                values
                    .iter()
                    .map(|value| from_rmpv_with_session(lua, value, Arc::clone(&session)))
                    .collect::<LuaResult<Vec<_>>>()?,
            )?,
        ),
        RmpvValue::Map(values) => {
            let table = lua.create_table()?;
            for (key, value) in values {
                table.set(
                    from_rmpv_with_session(lua, key, Arc::clone(&session))?,
                    from_rmpv_with_session(lua, value, Arc::clone(&session))?,
                )?;
            }
            LuaValue::Table(table)
        }
        RmpvValue::Ext(_, _) => match decode_resource_handle(value) {
            Ok(handle) if !handle.resource.is_empty() => LuaValue::UserData(lua.create_userdata(
                LuaWasmResource(Arc::new(ResourceState {
                    session,
                    id: handle.id,
                    owned: handle.owned,
                    interface: handle.interface,
                    resource: handle.resource,
                    alive: AtomicBool::new(true),
                })),
            )?),
            _ => from_rmpv(lua, value)?,
        },
        _ => from_rmpv(lua, value)?,
    })
}
