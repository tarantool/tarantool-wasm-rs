use std::collections::HashMap;

use crate::bindings::helpers::{state_lock_poisoned, unknown_error};
use crate::bindings::tarantool::tarantool::types::{BoxError, BoxTuple, KeyDef, TupleFormat};
use crate::ffi;
use crate::runtime::tarantool_wasm::SyncWasmStates;
use crate::runtime::tx::run_on_tx_detached;

#[derive(Clone, Copy)]
struct RefCountedHandle {
    ptr: u64,
    refs: u64,
    native_owned: bool,
}

pub(crate) struct NativeHandles {
    next: u64,
    tuples: HashMap<u64, RefCountedHandle>,
    key_defs: HashMap<u64, u64>,
    tuple_formats: HashMap<u64, RefCountedHandle>,
}

impl NativeHandles {
    pub(crate) fn new() -> Self {
        Self {
            next: 1,
            tuples: HashMap::new(),
            key_defs: HashMap::new(),
            tuple_formats: HashMap::new(),
        }
    }

    fn next_handle(&mut self) -> Result<u64, BoxError> {
        let handle = self.next;
        self.next = self
            .next
            .checked_add(1)
            .ok_or_else(|| unknown_error("native handle space exhausted"))?;
        Ok(handle)
    }

    fn insert_tuple(&mut self, ptr: u64) -> Result<BoxTuple, BoxError> {
        let handle = self.next_handle()?;
        self.tuples.insert(
            handle,
            RefCountedHandle {
                ptr,
                refs: 1,
                native_owned: true,
            },
        );
        Ok(BoxTuple { handle })
    }

    fn insert_key_def(&mut self, ptr: u64) -> Result<KeyDef, BoxError> {
        let handle = self.next_handle()?;
        self.key_defs.insert(handle, ptr);
        Ok(KeyDef { handle })
    }

    fn insert_tuple_format(
        &mut self,
        ptr: u64,
        native_owned: bool,
    ) -> Result<TupleFormat, BoxError> {
        let handle = self.next_handle()?;
        self.tuple_formats.insert(
            handle,
            RefCountedHandle {
                ptr,
                refs: 1,
                native_owned,
            },
        );
        Ok(TupleFormat { handle })
    }

    fn tuple_ptr(&self, handle: u64) -> Result<u64, BoxError> {
        self.tuples
            .get(&handle)
            .map(|entry| entry.ptr)
            .ok_or_else(|| invalid_handle("box-tuple", handle))
    }

    fn key_def_ptr(&self, handle: u64) -> Result<u64, BoxError> {
        self.key_defs
            .get(&handle)
            .copied()
            .ok_or_else(|| invalid_handle("key-def", handle))
    }

    fn tuple_format_ptr(&self, handle: u64) -> Result<u64, BoxError> {
        self.tuple_formats
            .get(&handle)
            .map(|entry| entry.ptr)
            .ok_or_else(|| invalid_handle("tuple-format", handle))
    }

    fn retain_tuple(&mut self, handle: u64) -> Result<u64, BoxError> {
        let entry = self
            .tuples
            .get_mut(&handle)
            .ok_or_else(|| invalid_handle("box-tuple", handle))?;
        entry.refs = entry
            .refs
            .checked_add(1)
            .ok_or_else(|| unknown_error("box-tuple reference count overflow"))?;
        Ok(entry.ptr)
    }

    fn release_tuple(&mut self, handle: u64) -> Result<u64, BoxError> {
        release_ref_counted(&mut self.tuples, "box-tuple", handle).map(|entry| entry.ptr)
    }

    fn retain_tuple_format(&mut self, handle: u64) -> Result<(u64, bool), BoxError> {
        let entry = self
            .tuple_formats
            .get_mut(&handle)
            .ok_or_else(|| invalid_handle("tuple-format", handle))?;
        entry.refs = entry
            .refs
            .checked_add(1)
            .ok_or_else(|| unknown_error("tuple-format reference count overflow"))?;
        Ok((entry.ptr, entry.native_owned))
    }

    fn release_tuple_format(&mut self, handle: u64) -> Result<(u64, bool), BoxError> {
        release_ref_counted(&mut self.tuple_formats, "tuple-format", handle)
            .map(|entry| (entry.ptr, entry.native_owned))
    }

    fn release_key_def(&mut self, handle: u64) -> Result<u64, BoxError> {
        self.key_defs
            .remove(&handle)
            .ok_or_else(|| invalid_handle("key-def", handle))
    }
}

fn release_ref_counted(
    entries: &mut HashMap<u64, RefCountedHandle>,
    kind: &'static str,
    handle: u64,
) -> Result<RefCountedHandle, BoxError> {
    let entry = entries
        .get_mut(&handle)
        .ok_or_else(|| invalid_handle(kind, handle))?;
    entry.refs -= 1;
    let released = *entry;
    if entry.refs == 0 {
        entries.remove(&handle);
    }
    Ok(released)
}

fn invalid_handle(kind: &str, handle: u64) -> BoxError {
    unknown_error(format!("invalid or stale {kind} handle {handle}"))
}

impl Drop for NativeHandles {
    fn drop(&mut self) {
        let tuples = self
            .tuples
            .drain()
            .map(|(_, entry)| entry)
            .collect::<Vec<_>>();
        let key_defs = self
            .key_defs
            .drain()
            .map(|(_, ptr)| ptr)
            .collect::<Vec<_>>();
        let tuple_formats = self
            .tuple_formats
            .drain()
            .map(|(_, entry)| entry)
            .filter(|entry| entry.native_owned)
            .collect::<Vec<_>>();

        run_on_tx_detached(move || unsafe {
            for entry in tuples {
                for _ in 0..entry.refs {
                    ffi::tuple::box_tuple_unref(entry.ptr as *mut _);
                }
            }
            for ptr in key_defs {
                ffi::tuple::box_key_def_delete(ptr as *mut _);
            }
            for entry in tuple_formats {
                for _ in 0..entry.refs {
                    ffi::tuple::box_tuple_format_unref(entry.ptr as *mut _);
                }
            }
        });
    }
}

impl SyncWasmStates {
    pub(crate) fn register_tuple(&self, ptr: u64) -> Result<BoxTuple, BoxError> {
        let result = match self.state.lock() {
            Ok(mut state) => state.handles.insert_tuple(ptr),
            Err(_) => Err(state_lock_poisoned()),
        };
        if result.is_err() {
            run_on_tx_detached(move || unsafe { ffi::tuple::box_tuple_unref(ptr as *mut _) });
        }
        result
    }

    pub(crate) fn register_key_def(&self, ptr: u64) -> Result<KeyDef, BoxError> {
        let result = match self.state.lock() {
            Ok(mut state) => state.handles.insert_key_def(ptr),
            Err(_) => Err(state_lock_poisoned()),
        };
        if result.is_err() {
            run_on_tx_detached(move || unsafe { ffi::tuple::box_key_def_delete(ptr as *mut _) });
        }
        result
    }

    pub(crate) fn register_tuple_format(
        &self,
        ptr: u64,
        native_owned: bool,
    ) -> Result<TupleFormat, BoxError> {
        let result = match self.state.lock() {
            Ok(mut state) => state.handles.insert_tuple_format(ptr, native_owned),
            Err(_) => Err(state_lock_poisoned()),
        };
        if result.is_err() && native_owned {
            run_on_tx_detached(move || unsafe {
                ffi::tuple::box_tuple_format_unref(ptr as *mut _)
            });
        }
        result
    }

    pub(crate) fn tuple_ptr(&self, tuple: BoxTuple) -> Result<u64, BoxError> {
        self.state
            .lock()
            .map_err(|_| state_lock_poisoned())?
            .handles
            .tuple_ptr(tuple.handle)
    }

    pub(crate) fn key_def_ptr(&self, key_def: KeyDef) -> Result<u64, BoxError> {
        self.state
            .lock()
            .map_err(|_| state_lock_poisoned())?
            .handles
            .key_def_ptr(key_def.handle)
    }

    pub(crate) fn tuple_format_ptr(&self, format: TupleFormat) -> Result<u64, BoxError> {
        self.state
            .lock()
            .map_err(|_| state_lock_poisoned())?
            .handles
            .tuple_format_ptr(format.handle)
    }

    pub(crate) fn retain_tuple_handle(&self, tuple: BoxTuple) -> Result<u64, BoxError> {
        self.state
            .lock()
            .map_err(|_| state_lock_poisoned())?
            .handles
            .retain_tuple(tuple.handle)
    }

    pub(crate) fn release_tuple_handle(&self, tuple: BoxTuple) -> Result<u64, BoxError> {
        self.state
            .lock()
            .map_err(|_| state_lock_poisoned())?
            .handles
            .release_tuple(tuple.handle)
    }

    pub(crate) fn retain_tuple_format_handle(
        &self,
        format: TupleFormat,
    ) -> Result<(u64, bool), BoxError> {
        self.state
            .lock()
            .map_err(|_| state_lock_poisoned())?
            .handles
            .retain_tuple_format(format.handle)
    }

    pub(crate) fn release_tuple_format_handle(
        &self,
        format: TupleFormat,
    ) -> Result<(u64, bool), BoxError> {
        self.state
            .lock()
            .map_err(|_| state_lock_poisoned())?
            .handles
            .release_tuple_format(format.handle)
    }

    pub(crate) fn release_key_def_handle(&self, key_def: KeyDef) -> Result<u64, BoxError> {
        self.state
            .lock()
            .map_err(|_| state_lock_poisoned())?
            .handles
            .release_key_def(key_def.handle)
    }
}

#[cfg(test)]
mod tests {
    use super::{release_ref_counted, RefCountedHandle};
    use std::collections::HashMap;

    #[test]
    fn removes_ref_counted_handle_only_after_last_release() {
        let mut entries = HashMap::from([(
            7,
            RefCountedHandle {
                ptr: 0x70,
                refs: 2,
                native_owned: true,
            },
        )]);

        let first = release_ref_counted(&mut entries, "box-tuple", 7).unwrap();
        assert_eq!(first.ptr, 0x70);
        assert_eq!(first.refs, 1);
        assert!(entries.contains_key(&7));

        let second = release_ref_counted(&mut entries, "box-tuple", 7).unwrap();
        assert_eq!(second.ptr, 0x70);
        assert_eq!(second.refs, 0);
        assert!(!entries.contains_key(&7));
    }
}
