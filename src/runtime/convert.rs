//! Conversion utilities between MessagePack values and Wasmtime component types.
//! These helpers are used by the public API to bridge Lua, MessagePack and
//! WebAssembly worlds.

use anyhow::Result;
use rmpv::Value as RmpvValue;
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use wasmtime::component::Type;
use wasmtime::component::Val;

const MSGPACK_EXT_TIMESTAMP: i8 = -1;
const TARANTOOL_EXT_DATETIME: i8 = 4;

/// Convert a [`rmpv::Value`] into a [`Val`] according to the expected [`Type`].
pub fn to_component_val(ty: &Type, val: &RmpvValue) -> Result<Val> {
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
                .ok_or_else(|| anyhow::anyhow!("expected s64"))?,
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
                .ok_or_else(|| anyhow::anyhow!("expected u64"))?,
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
            Val::List(list)
        }

        // Option
        Type::Option(inner_ty) => match val {
            RmpvValue::Nil => Val::Option(None),
            _ => {
                let inner = to_component_val(&inner_ty.ty(), val)?;
                Val::Option(Some(Box::new(inner)))
            }
        },

        // Tuple
        Type::Tuple(tuple_ty) => {
            let arr = val
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("expected array for tuple"))?;

            let tys = tuple_ty.types();

            if arr.len() != tys.len() {
                return Err(anyhow::anyhow!(
                    "tuple length mismatch: expected {}, got {}",
                    tys.len(),
                    arr.len()
                ));
            }

            let items = arr
                .iter()
                .zip(tys)
                .map(|(v, t)| to_component_val(&t, v))
                .collect::<Result<Vec<_>>>()?;

            Val::Tuple(items)
        }

        Type::Record(record_ty) => {
            let map = val
                .as_map()
                .ok_or_else(|| anyhow::anyhow!("expected map for record"))?;

            let mut map_by_name: HashMap<&str, &RmpvValue> = HashMap::with_capacity(map.len());
            for (key, value) in map {
                if let Some(name) = key.as_str() {
                    map_by_name.entry(name).or_insert(value);
                }
            }

            let mut fields = Vec::new();

            for field in record_ty.fields() {
                let val_item = if let Some(value) = map_by_name.get(field.name) {
                    *value
                } else if matches!(field.ty, Type::Option(_)) {
                    &RmpvValue::Nil
                } else {
                    return Err(anyhow::anyhow!("missing field `{}` in record", field.name));
                };

                let field_val = to_component_val(&field.ty, val_item)?;
                fields.push((field.name.into(), field_val));
            }

            Val::Record(fields)
        }

        Type::Variant(variant_ty) => {
            let map = val
                .as_map()
                .ok_or_else(|| anyhow::anyhow!("expected map for variant"))?;

            if map.len() != 1 {
                return Err(anyhow::anyhow!("expected exactly one key in variant"));
            }

            let (case_key, case_val) = map.iter().next().unwrap();

            let case_name = match case_key {
                RmpvValue::String(s) => s.as_str(),
                _ => None,
            }
            .ok_or_else(|| anyhow::anyhow!("variant key must be string"))?;

            let case = variant_ty
                .cases()
                .find(|c| c.name == case_name)
                .ok_or_else(|| anyhow::anyhow!("unknown variant case: {}", case_name))?;

            let val = match &case.ty {
                Some(ty) => {
                    let inner = to_component_val(ty, case_val)?;
                    Val::Variant(case.name.to_string(), Some(Box::new(inner)))
                }
                None => {
                    // allow 'true' and 'nil' as syntactic sugar for unit-like variants
                    match case_val {
                        RmpvValue::Boolean(true) | RmpvValue::Nil => {
                            Val::Variant(case.name.to_string(), None)
                        }
                        _ => {
                            return Err(anyhow::anyhow!(
                                "unexpected payload for unit variant `{}`: {:?}",
                                case.name,
                                case_val
                            ));
                        }
                    }
                }
            };

            val
        }

        Type::Enum(enum_ty) => {
            let s = val
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("expected string for enum"))?;

            let case = enum_ty
                .names()
                .find(|&name| name == s)
                .ok_or_else(|| anyhow::anyhow!("invalid enum case: {s}"))?;

            Val::Enum(case.to_string())
        }

        Type::Flags(flags_ty) => {
            let arr = val
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("expected array for flags"))?;

            let valid_flags: HashSet<&str> = flags_ty.names().collect();
            let mut active_flags = Vec::with_capacity(arr.len());

            for v in arr {
                let name = v
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("expected string in flags array"))?;
                if !valid_flags.contains(name) {
                    return Err(anyhow::anyhow!("invalid flag name: {}", name));
                }
                active_flags.push(name.to_string());
            }

            Val::Flags(active_flags)
        }

        _ => return Err(anyhow::anyhow!("unsupported param type: {:?}", ty)),
    })
}

/// Convert a [`Val`] produced by Wasmtime back into a MessagePack value.
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
            Some(inner) => val_to_rmpv(inner)?,
            None => RmpvValue::Nil,
        },
        Val::Result(r) => match r {
            Ok(Some(v)) => RmpvValue::Array(vec![RmpvValue::Boolean(true), val_to_rmpv(v)?]),
            Ok(None) => RmpvValue::Array(vec![RmpvValue::Boolean(true), RmpvValue::Nil]),
            Err(Some(e)) => RmpvValue::Array(vec![RmpvValue::Boolean(false), val_to_rmpv(e)?]),
            Err(None) => RmpvValue::Array(vec![RmpvValue::Boolean(false), RmpvValue::Nil]),
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

/// Encode an [`rmpv::Value`] into raw MessagePack bytes.
pub fn rmpv_to_msgpack_bytes(value: &RmpvValue) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, value)
        .map_err(|err| anyhow::anyhow!("failed to encode MessagePack: {err}"))?;
    Ok(buf)
}

/// Decode raw MessagePack bytes into an [`rmpv::Value`].
pub fn msgpack_bytes_to_rmpv(bytes: &[u8]) -> Result<RmpvValue> {
    let mut cursor = Cursor::new(bytes);
    rmpv::decode::read_value(&mut cursor)
        .map_err(|err| anyhow::anyhow!("failed to decode MessagePack: {err}"))
}

fn timestamp_ext_to_tarantool_datetime_payload(bytes: &[u8]) -> Result<Vec<u8>> {
    let (secs, nsec) = match bytes.len() {
        // timestamp32: 32-bit unsigned seconds (big-endian)
        4 => {
            let mut sec = [0_u8; 4];
            sec.copy_from_slice(bytes);
            (u32::from_be_bytes(sec) as i64, 0_u32)
        }
        // timestamp64: 30-bit nanoseconds + 34-bit unsigned seconds (big-endian)
        8 => {
            let mut raw = [0_u8; 8];
            raw.copy_from_slice(bytes);
            let packed = u64::from_be_bytes(raw);
            let nsec = (packed >> 34) as u32;
            let secs = (packed & ((1_u64 << 34) - 1)) as i64;
            (secs, nsec)
        }
        // timestamp96: 32-bit nanoseconds + 64-bit signed seconds (big-endian)
        12 => {
            let mut nsec_bytes = [0_u8; 4];
            nsec_bytes.copy_from_slice(&bytes[0..4]);
            let mut sec_bytes = [0_u8; 8];
            sec_bytes.copy_from_slice(&bytes[4..12]);
            (
                i64::from_be_bytes(sec_bytes),
                u32::from_be_bytes(nsec_bytes),
            )
        }
        len => {
            return Err(anyhow::anyhow!(
                "unsupported MsgPack timestamp payload length: {len}"
            ));
        }
    };

    if nsec >= 1_000_000_000 {
        return Err(anyhow::anyhow!(
            "invalid MsgPack timestamp nanoseconds value: {nsec}"
        ));
    }

    if nsec == 0 {
        return Ok(secs.to_le_bytes().to_vec());
    }

    let mut payload = vec![0_u8; 16];
    payload[0..8].copy_from_slice(&secs.to_le_bytes());
    payload[8..12].copy_from_slice(&nsec.to_le_bytes());
    payload[12..14].copy_from_slice(&0_i16.to_le_bytes()); // UTC offset in minutes.
    payload[14..16].copy_from_slice(&0_i16.to_le_bytes()); // tzindex
    Ok(payload)
}

fn normalize_msgpack_ext_for_tarantool(value: &mut RmpvValue) -> Result<()> {
    match value {
        RmpvValue::Array(items) => {
            for item in items {
                normalize_msgpack_ext_for_tarantool(item)?;
            }
        }
        RmpvValue::Map(entries) => {
            for (k, v) in entries {
                normalize_msgpack_ext_for_tarantool(k)?;
                normalize_msgpack_ext_for_tarantool(v)?;
            }
        }
        RmpvValue::Ext(tag, data) if *tag == MSGPACK_EXT_TIMESTAMP => {
            let converted = timestamp_ext_to_tarantool_datetime_payload(data)?;
            *tag = TARANTOOL_EXT_DATETIME;
            *data = converted;
        }
        _ => {}
    }

    Ok(())
}

/// Convert a [`mlua::Value`] into an [`rmpv::Value`].
fn lua_msgpack_module(lua: &mlua::Lua) -> mlua::Result<mlua::Table> {
    let require: mlua::Function = lua.globals().get("require")?;
    require.call("msgpack")
}

fn lua_value_to_rmpv_via_msgpack(lua: &mlua::Lua, val: &mlua::Value) -> mlua::Result<rmpv::Value> {
    let msgpack = lua_msgpack_module(lua)?;
    let encode: mlua::Function = msgpack.get("encode")?;
    let encoded: mlua::String = encode.call(val.clone())?;
    let bytes = encoded.as_bytes();
    msgpack_bytes_to_rmpv(bytes.as_ref()).map_err(mlua::Error::external)
}

fn rmpv_to_lua_via_msgpack(lua: &mlua::Lua, val: &rmpv::Value) -> mlua::Result<mlua::Value> {
    let mut normalized = val.clone();
    normalize_msgpack_ext_for_tarantool(&mut normalized).map_err(mlua::Error::external)?;
    let encoded = rmpv_to_msgpack_bytes(&normalized).map_err(mlua::Error::external)?;
    let msgpack = lua_msgpack_module(lua)?;
    let decode: mlua::Function = msgpack.get("decode")?;
    decode.call(lua.create_string(&encoded)?)
}

pub fn to_rmpv(lua: &mlua::Lua, val: &mlua::Value) -> Result<rmpv::Value, mlua::Error> {
    use mlua::Value::*;
    Ok(match val {
        Nil => rmpv::Value::Nil,
        Boolean(b) => rmpv::Value::Boolean(*b),
        Integer(i) => rmpv::Value::Integer((*i).into()),
        Number(n) => rmpv::Value::F64(*n),
        String(s) => {
            let str_val = s.to_str()?;
            rmpv::Value::String(rmpv::Utf8String::from(str_val.to_string()))
        }
        Table(t) => {
            // Try array-style first, fallback to map
            let is_array = t.clone().pairs::<mlua::Value, mlua::Value>().all(|p| {
                p.map(|(k, _)| matches!(k, mlua::Value::Integer(_)))
                    .unwrap_or(false)
            });

            if is_array {
                let mut vec = Vec::new();
                for pair in t.clone().sequence_values::<mlua::Value>() {
                    let v = pair?;
                    vec.push(to_rmpv(lua, &v)?);
                }
                rmpv::Value::Array(vec)
            } else {
                let mut map = Vec::new();
                for pair in t.clone().pairs::<mlua::Value, mlua::Value>() {
                    let (k, v) = pair?;
                    map.push((to_rmpv(lua, &k)?, to_rmpv(lua, &v)?));
                }
                rmpv::Value::Map(map)
            }
        }
        other => lua_value_to_rmpv_via_msgpack(lua, other)?,
    })
}

/// Convert a [`rmpv::Value`] into a Lua value using the provided [`Lua`] state.
pub fn from_rmpv(lua: &mlua::Lua, val: &rmpv::Value) -> mlua::Result<mlua::Value> {
    use mlua::Value as LV;
    use rmpv::Value::*;

    Ok(match val {
        Nil => LV::Nil,
        Boolean(b) => LV::Boolean(*b),
        Integer(i) => LV::Integer(i.as_i64().unwrap_or(0)),
        F32(f) => LV::Number(*f as f64),
        F64(f) => LV::Number(*f),
        String(s) => LV::String(lua.create_string(s.as_str().unwrap_or(""))?),
        Binary(bytes) => LV::String(lua.create_string(bytes)?),
        Array(arr) => {
            let seq = arr
                .iter()
                .map(|v| from_rmpv(lua, v))
                .collect::<mlua::Result<Vec<_>>>()?;
            LV::Table(lua.create_sequence_from(seq)?)
        }
        Map(map) => {
            let tbl = lua.create_table()?;
            for (k, v) in map {
                tbl.set(from_rmpv(lua, k)?, from_rmpv(lua, v)?)?;
            }
            LV::Table(tbl)
        }
        Ext(_, _) => rmpv_to_lua_via_msgpack(lua, val)?,
    })
}

/// Human readable string representation of a Wasmtime [`Type`].
pub fn pretty_type(ty: &Type) -> String {
    match ty {
        Type::Bool => "bool".into(),
        Type::U8 => "u8".into(),
        Type::U16 => "u16".into(),
        Type::U32 => "u32".into(),
        Type::U64 => "u64".into(),
        Type::S8 => "s8".into(),
        Type::S16 => "s16".into(),
        Type::S32 => "s32".into(),
        Type::S64 => "s64".into(),
        Type::Float32 => "f32".into(),
        Type::Float64 => "f64".into(),
        Type::Char => "char".into(),
        Type::String => "string".into(),

        Type::List(inner) => format!("list<{}>", pretty_type(&inner.ty())),
        Type::Option(inner) => format!("option<{}>", pretty_type(&inner.ty())),

        Type::Tuple(tuple) => {
            let inner = tuple
                .types()
                .map(|ty: Type| pretty_type(&ty))
                .collect::<Vec<_>>()
                .join(", ");
            format!("tuple<{inner}>")
        }

        Type::Record(rec) => {
            let inner = rec
                .fields()
                .map(|f| format!("{}: {}", f.name, pretty_type(&f.ty)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("record{{{inner}}}")
        }

        Type::Variant(var) => {
            let inner = var
                .cases()
                .map(|c| {
                    if let Some(t) = &c.ty {
                        format!("{}({})", c.name, pretty_type(t))
                    } else {
                        c.name.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(" | ");
            format!("variant({inner})")
        }

        Type::Enum(e) => {
            let names = e.names().collect::<Vec<_>>().join(" | ");
            format!("enum({names})")
        }

        Type::Flags(f) => {
            let names = f.names().collect::<Vec<_>>().join(", ");
            format!("flags({names})")
        }

        Type::Result(r) => {
            let ok = r.ok().as_ref().map_or("unit".into(), pretty_type);
            let err = r.err().as_ref().map_or("unit".into(), pretty_type);
            format!("result<{ok}, {err}>")
        }

        // Fallback
        other => format!("{other:?}"),
    }
}
