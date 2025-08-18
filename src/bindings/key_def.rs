use crate::bindings::error::handle_tarantool_error;
use crate::bindings::tarantool::tarantool::key_def;
use crate::bindings::tarantool::tarantool::types::{
    BoxError, BoxTuple, Collation, FieldType, KeyDef, KeyPart, KeyPartFlags,
};
use crate::ffi;
use crate::runtime::tarantool_wasm::SyncWasmStates;

use std::ffi::CString;
use std::mem::ManuallyDrop;
use std::str::FromStr;

use tarantool::error::TarantoolErrorCode;

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

fn decode_key_part_flags(bits: u32) -> KeyPartFlags {
    let mut raw: u8 = 0;
    if bits & (1 << 0) != 0 {
        raw |= 1 << 0; // is-nullable
    }
    if bits & (1 << 1) != 0 {
        raw |= 1 << 1; // exclude-null
    }

    unsafe { std::mem::transmute(raw) }
}

impl key_def::Host for SyncWasmStates {
    async fn new(&mut self, parts: Vec<KeyPart>) -> Result<KeyDef, BoxError> {
        let part_count = parts.len();

        let mut key_parts: Vec<ffi::types::BoxKeyPartDefT> = Vec::with_capacity(part_count);
        let mut c_field_types = Vec::with_capacity(part_count);
        let mut c_collations = Vec::with_capacity(part_count);
        let mut c_paths = Vec::with_capacity(part_count);

        for part in parts.iter() {
            let field_type_str = part.field_type.as_str();
            let field_type_cstr = CString::new(field_type_str).map_err(|_| {
                BoxError::new(
                    TarantoolErrorCode::IllegalParams as u32,
                    "Invalid field type",
                )
            })?;
            let field_type_ptr = field_type_cstr.as_ptr();
            c_field_types.push(field_type_cstr);

            let collation_ptr: *const libc::c_char = if let Some(ref collation) = part.collation {
                let collation_cstr = CString::new(collation.as_str()).map_err(|_| {
                    BoxError::new(
                        TarantoolErrorCode::IllegalParams as u32,
                        "Invalid field type",
                    )
                })?;
                let ptr = collation_cstr.as_ptr();
                c_collations.push(collation_cstr);
                ptr
            } else {
                std::ptr::null()
            };

            let path_ptr: *const libc::c_char = if let Some(ref path) = part.path {
                let path_cstr = CString::new(path.as_str()).map_err(|_| {
                    BoxError::new(TarantoolErrorCode::IllegalParams as u32, "Invalid path")
                })?;
                let ptr = path_cstr.as_ptr();
                c_paths.push(path_cstr);
                ptr
            } else {
                std::ptr::null()
            };

            let flags = encode_key_part_flags(&part.flags);

            let fields = ffi::types::BoxKeyPartDefFields {
                fieldno: part.field_no,
                flags,
                field_type: field_type_ptr,
                collation: collation_ptr,
                path: path_ptr,
            };

            key_parts.push(ffi::types::BoxKeyPartDefT {
                fields: ManuallyDrop::new(fields),
            });
        }

        let ptr =
            unsafe { ffi::tuple::box_key_def_new_v2(key_parts.as_mut_ptr(), part_count as u32) };

        if ptr.is_null() {
            Err(handle_tarantool_error())
        } else {
            Ok(KeyDef { ptr: ptr as u64 })
        }
    }

    async fn delete(&mut self, key_def: KeyDef) {
        let ptr = key_def.ptr as *mut ffi::types::BoxKeyDefT;
        unsafe {
            ffi::tuple::box_key_def_delete(ptr);
        }
    }

    async fn merge(&mut self, left: KeyDef, right: KeyDef) -> Result<KeyDef, BoxError> {
        let ptr_left = left.ptr as *const ffi::types::BoxKeyDefT;
        let ptr_right = right.ptr as *const ffi::types::BoxKeyDefT;

        let ptr = unsafe { ffi::tuple::box_key_def_merge(ptr_left, ptr_right) };

        if ptr.is_null() {
            Err(handle_tarantool_error())
        } else {
            Ok(KeyDef { ptr: ptr as u64 })
        }
    }

    async fn dup(&mut self, key_def: KeyDef) -> Result<KeyDef, BoxError> {
        let ptr = key_def.ptr as *const ffi::types::BoxKeyDefT;
        if ptr.is_null() {
            return Err(BoxError::new(
                TarantoolErrorCode::Unknown as u32,
                "Invalid key-def",
            ));
        }
        let new_ptr = unsafe { ffi::tuple::box_key_def_dup(ptr) };
        Ok(KeyDef {
            ptr: new_ptr as u64,
        })
    }

    async fn dump_parts(&mut self, key_def: KeyDef) -> Result<Vec<KeyPart>, BoxError> {
        let mut part_count: u32 = 0;
        let raw_ptr = unsafe {
            ffi::tuple::box_key_def_dump_parts(key_def.ptr as *const _, &mut part_count as *mut _)
        };

        if raw_ptr.is_null() {
            return Err(handle_tarantool_error());
        }

        if part_count == 0 {
            return Ok(Vec::new());
        }

        let part_size = std::mem::size_of::<ffi::types::BoxKeyPartDefT>();

        // Copy raw bytes
        let src = raw_ptr as *const u8;
        let mut result = Vec::with_capacity(part_count as usize);

        for i in 0..part_count {
            let offset = i as isize * part_size as isize;
            let part_ptr = unsafe { src.offset(offset) as *const ffi::types::BoxKeyPartDefT };
            let part_union = unsafe { std::ptr::read_unaligned(part_ptr) };
            let part = unsafe { &part_union.fields };

            let field_no = part.fieldno;
            let flags = decode_key_part_flags(part.flags);

            let field_type = unsafe {
                std::ffi::CStr::from_ptr(part.field_type)
                    .to_string_lossy()
                    .to_string()
            };

            let collation_str_opt: Option<String> = unsafe {
                if part.collation.is_null() {
                    None
                } else {
                    Some(
                        std::ffi::CStr::from_ptr(part.collation)
                            .to_string_lossy()
                            .into_owned(),
                    )
                }
            };

            let path: Option<String> = unsafe {
                if part.path.is_null() {
                    None
                } else {
                    Some(
                        std::ffi::CStr::from_ptr(part.path)
                            .to_string_lossy()
                            .into_owned(),
                    )
                }
            };

            let field_type = FieldType::from_str(&field_type)?;

            let collation: Option<Collation> = match collation_str_opt.as_deref() {
                None => None,
                Some(s) => Some(Collation::from_str(s)?),
            };

            result.push(KeyPart {
                field_no,
                field_type,
                collation,
                path,
                flags,
            });
        }

        Ok(result)
    }

    async fn validate_key(
        &mut self,
        key_def: KeyDef,
        key: Vec<u8>,
    ) -> Result<(bool, u32), BoxError> {
        let key_ptr = key.as_ptr() as *const libc::c_char;
        let mut key_size: u32 = 0;

        let rc = unsafe {
            ffi::tuple::box_key_def_validate_key(key_def.ptr as *const _, key_ptr, &mut key_size)
        };

        if rc == 0 {
            Ok((true, key_size))
        } else {
            Err(handle_tarantool_error())
        }
    }

    async fn validate_full_key(
        &mut self,
        key_def: KeyDef,
        key: Vec<u8>,
    ) -> Result<(bool, u32), BoxError> {
        let key_ptr = key.as_ptr() as *const libc::c_char;
        let mut key_size: u32 = 0;

        let rc = unsafe {
            ffi::tuple::box_key_def_validate_full_key(
                key_def.ptr as *const _,
                key_ptr,
                &mut key_size,
            )
        };

        if rc == 0 {
            Ok((true, key_size))
        } else {
            Err(handle_tarantool_error())
        }
    }

    async fn part_count(&mut self, key_def: KeyDef) -> Result<u32, BoxError> {
        let mut count: u32 = 0;

        let ptr = unsafe {
            ffi::tuple::box_key_def_dump_parts(key_def.ptr as *const _, &mut count as *mut _)
        };

        if ptr.is_null() {
            Err(handle_tarantool_error())
        } else {
            Ok(count)
        }
    }

    async fn validate_tuple(&mut self, key_def: KeyDef, tuple: BoxTuple) -> bool {
        unsafe {
            ffi::tuple::box_key_def_validate_tuple(key_def.ptr as *mut _, tuple.ptr as *mut _) == 0
        }
    }

    async fn extract_key(&mut self, key_def: KeyDef, tuple: BoxTuple) -> Result<Vec<u8>, BoxError> {
        let mut size: u32 = 0;
        let ptr = unsafe {
            ffi::tuple::box_key_def_extract_key(
                key_def.ptr as *mut _,
                tuple.ptr as *mut _,
                -1,
                &mut size as *mut u32,
            )
        };

        if ptr.is_null() {
            return Err(handle_tarantool_error());
        }

        let slice = unsafe { std::slice::from_raw_parts(ptr as *const u8, size as usize) };
        Ok(slice.to_vec())
    }
}
