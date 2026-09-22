use crate::runtime::tarantool_wasm::WasmConfig;
use mlua::prelude::*;
use std::net::IpAddr;
use wasmtime_wasi::FsPerms;

fn invalid_type(field: &str, expected: &str, value: &LuaValue) -> LuaError {
    LuaError::runtime(format!(
        "invalid `{field}`: expected {expected}, got {}",
        value.type_name()
    ))
}

fn optional_bool(tbl: &LuaTable, field: &str) -> LuaResult<Option<bool>> {
    match tbl.get::<LuaValue>(field)? {
        LuaValue::Nil => Ok(None),
        LuaValue::Boolean(value) => Ok(Some(value)),
        value => Err(invalid_type(field, "boolean", &value)),
    }
}

fn optional_string(tbl: &LuaTable, field: &str) -> LuaResult<Option<String>> {
    match tbl.get::<LuaValue>(field)? {
        LuaValue::Nil => Ok(None),
        LuaValue::String(value) => Ok(Some(value.to_str()?.to_string())),
        value => Err(invalid_type(field, "string", &value)),
    }
}

fn optional_table(tbl: &LuaTable, field: &str) -> LuaResult<Option<LuaTable>> {
    match tbl.get::<LuaValue>(field)? {
        LuaValue::Nil => Ok(None),
        LuaValue::Table(value) => Ok(Some(value)),
        value => Err(invalid_type(field, "table", &value)),
    }
}

fn unsigned_integer(value: LuaValue, field: &str) -> LuaResult<u64> {
    match value {
        LuaValue::Integer(value) => u64::try_from(value).map_err(|_| {
            LuaError::runtime(format!("invalid `{field}`: expected non-negative integer"))
        }),
        LuaValue::Number(value)
            if value.is_finite()
                && value >= 0.0
                && value.fract() == 0.0
                && value < u64::MAX as f64 =>
        {
            Ok(value as u64)
        }
        value => Err(invalid_type(field, "non-negative integer", &value)),
    }
}

fn optional_u64(tbl: &LuaTable, field: &str) -> LuaResult<Option<u64>> {
    match tbl.get::<LuaValue>(field)? {
        LuaValue::Nil => Ok(None),
        value => unsigned_integer(value, field).map(Some),
    }
}

fn required_string(value: LuaValue, field: &str) -> LuaResult<String> {
    match value {
        LuaValue::String(value) => Ok(value.to_str()?.to_string()),
        value => Err(invalid_type(field, "string", &value)),
    }
}

/// Parse a Lua table into a [`WasmConfig`].
///
/// Every configuration option is optional; if a field is absent the default
/// value from [`WasmConfig::default`] is used.
pub(super) fn parse_config_table(tbl: Option<LuaTable>) -> LuaResult<WasmConfig> {
    let mut cfg = WasmConfig::default();
    if let Some(t) = tbl {
        if let Some(value) = optional_bool(&t, "inherit_env")? {
            cfg.inherit_env = value;
        }
        if let Some(value) = optional_bool(&t, "inherit_args")? {
            cfg.inherit_args = value;
        }
        if let Some(args_tbl) = optional_table(&t, "args")? {
            for value in args_tbl.sequence_values::<String>() {
                cfg.args.push(value?);
            }
        }
        if let Some(value) = optional_bool(&t, "inherit_stdin")? {
            cfg.inherit_stdin = value;
        }
        if let Some(value) = optional_bool(&t, "inherit_stdout")? {
            cfg.inherit_stdout = value;
        }
        if let Some(value) = optional_bool(&t, "inherit_stderr")? {
            cfg.inherit_stderr = value;
        }
        cfg.stdin = optional_string(&t, "stdin")?;
        cfg.stdout = optional_string(&t, "stdout")?;
        cfg.stderr = optional_string(&t, "stderr")?;
        if let Some(env_tbl) = optional_table(&t, "env")? {
            for pair in env_tbl.pairs::<LuaValue, LuaValue>() {
                let (key, value) = pair?;
                let key = required_string(key, "env key")?;
                let value = required_string(value, "env value")?;
                cfg.env.insert(key, value);
            }
        }
        if let Some(value) = optional_bool(&t, "inherit_network")? {
            cfg.inherit_network = value;
        }
        if let Some(value) = optional_bool(&t, "allow_ip_name_lookup")? {
            cfg.allow_ip_name_lookup = value;
        }
        if let Some(value) = optional_bool(&t, "allow_tcp")? {
            cfg.allow_tcp = value;
        }
        if let Some(value) = optional_bool(&t, "allow_udp")? {
            cfg.allow_udp = value;
        }
        if let Some(ip_tbl) = optional_table(&t, "allowed_ips")? {
            for value in ip_tbl.sequence_values::<LuaValue>() {
                let ip = required_string(value?, "allowed_ips entry")?;
                ip.parse::<IpAddr>().map_err(|err| {
                    LuaError::runtime(format!("invalid IP address `{ip}`: {err}"))
                })?;
                cfg.allowed_ips.push(ip);
            }
        }
        if let Some(port_tbl) = optional_table(&t, "allowed_ports")? {
            for value in port_tbl.sequence_values::<LuaValue>() {
                let port = unsigned_integer(value?, "allowed_ports entry")?;
                let port = u16::try_from(port).map_err(|_| {
                    LuaError::runtime(format!(
                        "invalid `allowed_ports entry`: port {port} is out of range"
                    ))
                })?;
                cfg.allowed_ports.push(port);
            }
        }
        if let Some(po_tbl) = optional_table(&t, "preopened_dirs")? {
            for entry in po_tbl.sequence_values::<LuaValue>() {
                let entry = entry?;
                let LuaValue::Table(entry) = entry else {
                    return Err(invalid_type("preopened_dirs entry", "table", &entry));
                };
                let host = required_string(entry.raw_get::<LuaValue>(1)?, "preopened_dirs host")?;
                let guest = required_string(entry.raw_get::<LuaValue>(2)?, "preopened_dirs guest")?;
                let perms = optional_string(&entry, "perms")?;
                let perms = match perms.as_deref() {
                    Some("ro") => FsPerms::ReadOnly,
                    Some("rw") | None => FsPerms::ReadWrite,
                    Some(perms) => {
                        return Err(LuaError::runtime(format!(
                        "invalid preopened directory permissions `{perms}`; expected `ro` or `rw`"
                    )))
                    }
                };
                cfg.preopened_dirs.push((host, guest, perms));
            }
        }
        if let Some(memory_limit) = optional_u64(&t, "memory_limit")? {
            cfg.memory_limit = Some(usize::try_from(memory_limit).map_err(|_| {
                LuaError::runtime(format!(
                    "invalid `memory_limit`: {memory_limit} is out of range"
                ))
            })?);
        }
        cfg.max_instructions = optional_u64(&t, "max_instructions")?;
        if let Some(value) = optional_bool(&t, "interpreter")? {
            cfg.interpreter = value;
        }
        if let Some(value) = optional_bool(&t, "precompiled")? {
            cfg.precompiled = value;
        }
    }
    Ok(cfg)
}
