use crate::runtime::tarantool_wasm::WasmConfig;
use mlua::prelude::*;
use wasmtime_wasi::{DirPerms, FilePerms};

/// Parse a Lua table into a [`WasmConfig`].
///
/// Every configuration option is optional; if a field is absent the default
/// value from [`WasmConfig::default`] is used.
pub(super) fn parse_config_table(tbl: Option<LuaTable>) -> LuaResult<WasmConfig> {
    let mut cfg = WasmConfig::default();
    if let Some(t) = tbl {
        if let Ok(Some(inherit_env)) = t.get::<Option<bool>>("inherit_env") {
            cfg.inherit_env = inherit_env;
        }
        if let Ok(Some(inherit_args)) = t.get::<Option<bool>>("inherit_args") {
            cfg.inherit_args = inherit_args;
        }
        if let Ok(args_tbl) = t.get::<LuaTable>("args") {
            for val in args_tbl.sequence_values::<String>() {
                cfg.args.push(val?);
            }
        }
        if let Ok(Some(inherit_stdin)) = t.get::<Option<bool>>("inherit_stdin") {
            cfg.inherit_stdin = inherit_stdin;
        }
        if let Ok(Some(inherit_stdout)) = t.get::<Option<bool>>("inherit_stdout") {
            cfg.inherit_stdout = inherit_stdout;
        }
        if let Ok(Some(inherit_stderr)) = t.get::<Option<bool>>("inherit_stderr") {
            cfg.inherit_stderr = inherit_stderr;
        }
        if let Ok(stdin) = t.get::<String>("stdin") {
            cfg.stdin = Some(stdin);
        }
        if let Ok(stdout) = t.get::<String>("stdout") {
            cfg.stdout = Some(stdout);
        }
        if let Ok(stderr) = t.get::<String>("stderr") {
            cfg.stderr = Some(stderr);
        }
        if let Ok(env_tbl) = t.get::<LuaTable>("env") {
            for pair in env_tbl.pairs::<String, String>() {
                let (k, v) = pair?;
                cfg.env.insert(k, v);
            }
        }
        if let Ok(Some(inherit_network)) = t.get::<Option<bool>>("inherit_network") {
            cfg.inherit_network = inherit_network;
        }
        if let Ok(Some(allow_ip_name_lookup)) = t.get::<Option<bool>>("allow_ip_name_lookup") {
            cfg.allow_ip_name_lookup = allow_ip_name_lookup;
        }
        if let Ok(Some(allow_tcp)) = t.get::<Option<bool>>("allow_tcp") {
            cfg.allow_tcp = allow_tcp;
        }
        if let Ok(Some(allow_udp)) = t.get::<Option<bool>>("allow_udp") {
            cfg.allow_udp = allow_udp;
        }
        if let Ok(ip_tbl) = t.get::<LuaTable>("allowed_ips") {
            for ip in ip_tbl.sequence_values::<String>() {
                cfg.allowed_ips.push(ip?);
            }
        }
        if let Ok(port_tbl) = t.get::<LuaTable>("allowed_ports") {
            for port in port_tbl.sequence_values::<u16>() {
                cfg.allowed_ports.push(port?);
            }
        }
        if let Ok(po_tbl) = t.get::<LuaTable>("preopened_dirs") {
            for entry in po_tbl.sequence_values::<LuaTable>() {
                let tbl = entry?;
                let host: String = tbl.raw_get(1)?;
                let guest: String = tbl.raw_get(2)?;

                let perms: Option<String> = tbl.raw_get("perms").ok();
                let (dir_perms, file_perms) = match perms.as_deref() {
                    Some("ro") => (DirPerms::READ, FilePerms::READ),
                    _ => (DirPerms::all(), FilePerms::all()),
                };

                cfg.preopened_dirs
                    .push((host, guest, dir_perms, file_perms));
            }
        }
        if let Ok(memory_limit) = t.get::<u64>("memory_limit") {
            cfg.memory_limit = Some(memory_limit as usize);
        }
        if let Ok(max_instructions) = t.get::<u64>("max_instructions") {
            cfg.max_instructions = Some(max_instructions);
        }
        if let Ok(Some(interpreter)) = t.get::<Option<bool>>("interpreter") {
            cfg.interpreter = interpreter;
        }
        if let Ok(links_tbl) = t.get::<LuaTable>("links") {
            for pair in links_tbl.pairs::<String, String>() {
                let (import_name, module_uid) = pair?;
                cfg.links.insert(import_name, module_uid);
            }
        }
    }
    Ok(cfg)
}
