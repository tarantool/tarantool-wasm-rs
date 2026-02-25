use crate::runtime::convert::to_rmpv;
use mlua::prelude::*;
use mlua::Value;
use rmpv::Value as RmpvValue;

fn conversion_error(expected_to: &'static str, message: &'static str) -> mlua::Error {
    mlua::Error::FromLuaConversionError {
        from: "Value",
        to: expected_to.to_string(),
        message: Some(message.to_string()),
    }
}

pub(super) fn parse_required_string(
    value: Option<Value>,
    message: &'static str,
) -> LuaResult<String> {
    match value {
        Some(Value::String(s)) => Ok(s.to_str()?.to_string()),
        _ => Err(conversion_error("String", message)),
    }
}

pub(super) fn parse_function_selector(value: Option<Value>) -> LuaResult<(Option<String>, String)> {
    let selector = parse_required_string(value, "expected function name as string")?;
    Ok(split_function_selector(&selector))
}

pub(super) fn split_function_selector(selector: &str) -> (Option<String>, String) {
    match selector.split_once("::") {
        Some((iface, func)) => (Some(iface.to_string()), func.to_string()),
        None => (None, selector.to_string()),
    }
}

pub(super) fn parse_calls_table(value: Option<Value>) -> LuaResult<LuaTable> {
    match value {
        Some(Value::Table(table)) => Ok(table),
        _ => Err(conversion_error("table", "expected calls table")),
    }
}

pub(super) fn parse_batch_calls(calls_tbl: LuaTable) -> LuaResult<Vec<BatchCall>> {
    let mut calls = Vec::new();
    for pair in calls_tbl.sequence_values::<LuaValue>() {
        let val = pair?;
        let call_tbl = match val {
            LuaValue::Table(table) => table,
            _ => return Err(conversion_error("table", "each call must be table")),
        };

        let (interface, func) = match call_tbl.raw_get(1)? {
            LuaValue::String(s) => split_function_selector(s.to_str()?.as_ref()),
            _ => {
                return Err(conversion_error(
                    "string",
                    "call entry first element must be a function name string",
                ))
            }
        };

        let mut args_vals = Vec::new();
        for idx in 2..=call_tbl.raw_len() {
            args_vals.push(call_tbl.raw_get(idx)?);
        }

        let rmp_args = args_vals
            .iter()
            .map(to_rmpv)
            .collect::<Result<Vec<_>, _>>()?;

        calls.push((interface, func, rmp_args));
    }

    Ok(calls)
}

pub(super) type BatchCall = (Option<String>, String, Vec<RmpvValue>);
