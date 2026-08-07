use crate::bindings::error::handle_tarantool_error;
use crate::bindings::helpers::{
    debug_assert_tx_thread, illegal_params, validate_array, BoxRegionGuard,
};
use crate::bindings::tarantool::tarantool::key_def;
use crate::bindings::tarantool::tarantool::types::{
    BoxError, BoxTuple, Collation, FieldType, KeyDef, KeyPart, KeyPartFlags,
};
use crate::ffi;
use crate::runtime::tarantool_wasm::SyncWasmStates;

use std::ffi::CString;
use std::mem::ManuallyDrop;
use std::str::FromStr;

fn encode_key_part_flags(flags: &KeyPartFlags) -> u32 {
    let mut result = 0;
    if flags.contains(KeyPartFlags::IS_NULLABLE) {
        result |= 1 << 0; // is-nullable
    }
    if flags.contains(KeyPartFlags::EXCLUDE_NULL) {
        result |= 1 << 1; // exclude-null
    }

    result
}

fn decode_key_part_flags(bits: u32) -> KeyPartFlags {
    let mut flags = KeyPartFlags::empty();
    if bits & (1 << 0) != 0 {
        flags |= KeyPartFlags::IS_NULLABLE;
    }
    if bits & (1 << 1) != 0 {
        flags |= KeyPartFlags::EXCLUDE_NULL;
    }
    flags
}

impl key_def::Host for SyncWasmStates {
    async fn new(&mut self, parts: Vec<KeyPart>) -> Result<KeyDef, BoxError> {
        let ptr = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                let part_count = parts.len();

                let mut key_parts: Vec<ffi::types::BoxKeyPartDefT> = Vec::with_capacity(part_count);
                let mut c_field_types = Vec::with_capacity(part_count);
                let mut c_collations = Vec::with_capacity(part_count);
                let mut c_paths = Vec::with_capacity(part_count);

                for part in parts.iter() {
                    let field_type_str = part.field_type.as_str();
                    let field_type_cstr = CString::new(field_type_str)
                        .map_err(|_| illegal_params("Invalid field type"))?;
                    let field_type_ptr = field_type_cstr.as_ptr();
                    c_field_types.push(field_type_cstr);

                    let collation_ptr: *const libc::c_char =
                        if let Some(ref collation) = part.collation {
                            let collation_cstr = CString::new(collation.as_str())
                                .map_err(|_| illegal_params("Invalid field type"))?;
                            let ptr = collation_cstr.as_ptr();
                            c_collations.push(collation_cstr);
                            ptr
                        } else {
                            std::ptr::null()
                        };

                    let path_ptr: *const libc::c_char = if let Some(ref path) = part.path {
                        let path_cstr = CString::new(path.as_str())
                            .map_err(|_| illegal_params("Invalid path"))?;
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

                let ptr = unsafe {
                    ffi::tuple::box_key_def_new_v2(key_parts.as_mut_ptr(), part_count as u32)
                };

                if ptr.is_null() {
                    Err(handle_tarantool_error())
                } else {
                    Ok(ptr as u64)
                }
            })
            .await?;
        self.register_key_def(ptr)
    }

    async fn release(&mut self, key_def: KeyDef) -> wasmtime::Result<()> {
        let ptr = self
            .release_key_def_handle(key_def)
            .map_err(|err| wasmtime::Error::msg(err.message))?;
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                unsafe {
                    ffi::tuple::box_key_def_delete(ptr as *mut ffi::types::BoxKeyDefT);
                }
            })
            .await;
        Ok(())
    }

    async fn merge(&mut self, left: KeyDef, right: KeyDef) -> Result<KeyDef, BoxError> {
        let left = self.key_def_ptr(left)?;
        let right = self.key_def_ptr(right)?;
        let ptr = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                let ptr =
                    unsafe { ffi::tuple::box_key_def_merge(left as *const _, right as *const _) };
                if ptr.is_null() {
                    Err(handle_tarantool_error())
                } else {
                    Ok(ptr as u64)
                }
            })
            .await?;
        self.register_key_def(ptr)
    }

    async fn duplicate(&mut self, key_def: KeyDef) -> Result<KeyDef, BoxError> {
        let key_def = self.key_def_ptr(key_def)?;
        let ptr = self
            .executor()
            .execute(move || {
                debug_assert_tx_thread();
                let new_ptr = unsafe { ffi::tuple::box_key_def_dup(key_def as *const _) };
                if new_ptr.is_null() {
                    Err(handle_tarantool_error())
                } else {
                    Ok(new_ptr as u64)
                }
            })
            .await?;
        self.register_key_def(ptr)
    }

    async fn dump_parts(&mut self, key_def: KeyDef) -> Result<Vec<KeyPart>, BoxError> {
        let key_def = self.key_def_ptr(key_def)?;
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let _region = BoxRegionGuard::new();
                let mut part_count: u32 = 0;
                let raw_ptr = unsafe {
                    ffi::tuple::box_key_def_dump_parts(
                        key_def as *const _,
                        &mut part_count as *mut _,
                    )
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
                    let part_ptr =
                        unsafe { src.offset(offset) as *const ffi::types::BoxKeyPartDefT };
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
            })
            .await
    }

    async fn validate_key(&mut self, key_def: KeyDef, key: Vec<u8>) -> Result<(), BoxError> {
        validate_array(&key)?;
        let key_def = self.key_def_ptr(key_def)?;
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let key_ptr = key.as_ptr() as *const libc::c_char;
                let mut key_size: u32 = 0;
                let rc = unsafe {
                    ffi::tuple::box_key_def_validate_key(
                        key_def as *const _,
                        key_ptr,
                        &mut key_size,
                    )
                };
                if rc == 0 {
                    Ok(())
                } else {
                    Err(handle_tarantool_error())
                }
            })
            .await
    }

    async fn validate_full_key(&mut self, key_def: KeyDef, key: Vec<u8>) -> Result<(), BoxError> {
        validate_array(&key)?;
        let key_def = self.key_def_ptr(key_def)?;
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let key_ptr = key.as_ptr() as *const libc::c_char;
                let mut key_size: u32 = 0;
                let rc = unsafe {
                    ffi::tuple::box_key_def_validate_full_key(
                        key_def as *const _,
                        key_ptr,
                        &mut key_size,
                    )
                };
                if rc == 0 {
                    Ok(())
                } else {
                    Err(handle_tarantool_error())
                }
            })
            .await
    }

    async fn part_count(&mut self, key_def: KeyDef) -> Result<u32, BoxError> {
        let key_def = self.key_def_ptr(key_def)?;
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let _region = BoxRegionGuard::new();
                let mut count: u32 = 0;
                let ptr = unsafe {
                    ffi::tuple::box_key_def_dump_parts(key_def as *const _, &mut count as *mut _)
                };
                if ptr.is_null() {
                    Err(handle_tarantool_error())
                } else {
                    Ok(count)
                }
            })
            .await
    }

    async fn validate_tuple(&mut self, key_def: KeyDef, tuple: BoxTuple) -> Result<(), BoxError> {
        let key_def = self.key_def_ptr(key_def)?;
        let tuple = self.tuple_ptr(tuple)?;
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let rc = unsafe {
                    ffi::tuple::box_key_def_validate_tuple(key_def as *mut _, tuple as *mut _)
                };
                if rc == 0 {
                    Ok(())
                } else {
                    Err(handle_tarantool_error())
                }
            })
            .await
    }

    async fn extract_key(&mut self, key_def: KeyDef, tuple: BoxTuple) -> Result<Vec<u8>, BoxError> {
        let key_def = self.key_def_ptr(key_def)?;
        let tuple = self.tuple_ptr(tuple)?;
        self.executor()
            .execute(move || {
                debug_assert_tx_thread();
                let _region = BoxRegionGuard::new();
                let mut size: u32 = 0;
                let ptr = unsafe {
                    ffi::tuple::box_key_def_extract_key(
                        key_def as *mut _,
                        tuple as *mut _,
                        -1,
                        &mut size as *mut u32,
                    )
                };

                if ptr.is_null() {
                    return Err(handle_tarantool_error());
                }

                let slice = unsafe { std::slice::from_raw_parts(ptr as *const u8, size as usize) };
                Ok(slice.to_vec())
            })
            .await
    }
}
