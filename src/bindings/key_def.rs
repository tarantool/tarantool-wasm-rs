use crate::bindings::docs::adder::key_def;
use crate::bindings::docs::adder::types::{BoxError, KeyPart, KeyDef, KeyPartFlags};
use crate::tarantool_wasm::SyncWasmStates;
use crate::ffi as ffi;
use crate::bindings::error::handle_tarantool_error;

use wasmtime_wasi::async_trait;
use std::ffi::CString;

fn field_type_to_id(field_type: &str) -> Result<&'static str, BoxError> {
    match field_type {
        "unsigned" => Ok("unsigned"),
        "string" => Ok("string"),
        "number" => Ok("number"),
        "double" => Ok("double"),
        "integer" => Ok("integer"),
        "boolean" => Ok("boolean"),
        "varbinary" => Ok("varbinary"),
        "scalar" => Ok("scalar"),
        "decimal" => Ok("decimal"),
        "array" => Ok("array"),
        _ => Err(BoxError::new("Unsupported field type", "validation error", 2)),
    }
}

fn encode_key_part_flags(flags: &KeyPartFlags) -> u32 {
    let raw: u8 = unsafe { std::mem::transmute_copy(flags) };

    let mut result = 0;
    if raw & (1 << 0) != 0 {
        result |= 1 << 0; // is-nullable
    }
    if raw & (1 << 1) != 0 {
        result |= 1 << 1; // exclude-null
    }

    result
}


#[async_trait]
impl key_def::Host for SyncWasmStates {
    async fn new(&mut self, parts: Vec<KeyPart>) -> Result<KeyDef, BoxError> {
        let part_count = parts.len();
        println!("Creating key_def with {} parts", part_count);

        let mut key_parts: Vec<ffi::types::BoxKeyPartDefT> = Vec::with_capacity(part_count);
        let mut c_field_types = Vec::with_capacity(part_count);
        let mut c_collations = Vec::with_capacity(part_count);
        let mut c_paths = Vec::with_capacity(part_count);

        for (i, part) in parts.iter().enumerate() {
            println!("Part {}: {:?}", i, part);

            let mut def: ffi::types::BoxKeyPartDefT = unsafe { std::mem::zeroed() };
            unsafe { ffi::tuple::box_key_part_def_create(&mut def) };

            def.fieldno = part.field_no;
            def.flags = encode_key_part_flags(&part.flags);

            let field_type_str = field_type_to_id(&part.field_type)?;
            let field_type_cstr = CString::new(field_type_str)
                .map_err(|_| BoxError::new("Invalid field_type", "validation error", 2))?;

            def.field_type = field_type_cstr.as_ptr();
            c_field_types.push(field_type_cstr);

            if let Some(ref collation) = part.collation {
                let collation_cstr = CString::new(collation.as_str())
                    .map_err(|_| BoxError::new("Invalid collation", "validation error", 2))?;
                def.collation = collation_cstr.as_ptr();
                c_collations.push(collation_cstr);
            }

            if let Some(ref path) = part.path {
                let path_cstr = CString::new(path.as_str())
                    .map_err(|_| BoxError::new("Invalid path", "validation error", 2))?;
                def.path = path_cstr.as_ptr();
                c_paths.push(path_cstr);
            }

            key_parts.push(def);
        }

        let ptr = unsafe {
            ffi::tuple::box_key_def_new_v2(key_parts.as_mut_ptr(), part_count as u32)
        };

        if ptr.is_null() {
            Err(unsafe { handle_tarantool_error() })
        } else {
            println!("Successfully created key_def at ptr: {:?}", ptr);
            Ok(ptr as u64)
        }
    }    

    async fn delete(&mut self, key_def: KeyDef) {
        let ptr = key_def as *mut ffi::types::BoxKeyDefT;
        unsafe {
            ffi::tuple::box_key_def_delete(ptr);
        }
    }

    async fn merge(&mut self, left: KeyDef, right: KeyDef) -> Result<KeyDef, BoxError> {
        let ptr_left = left as *const ffi::types::BoxKeyDefT;
        let ptr_right = right as *const ffi::types::BoxKeyDefT;

        let ptr = unsafe {
            ffi::tuple::box_key_def_merge(ptr_left, ptr_right)
        };

        if ptr.is_null() {
            Err(unsafe{ handle_tarantool_error() })
        } else {
            Ok(ptr as KeyDef)
        }
    }
}