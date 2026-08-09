//! Dynamically loaded Tarantool Enterprise raw read-view C API.
//!
//! The symbols are resolved at runtime so the module remains loadable on
//! Tarantool Community Edition, where this API is not exported.

use libc::{c_char, c_int, c_void};
use std::ffi::CStr;
use std::sync::OnceLock;

const ITERATOR_SIZE: usize = 256;
const ENTERPRISE_MARKER: &CStr = c"box_raw_read_view_new";

#[repr(C)]
pub struct ReadView {
    _private: [u8; 0],
}

#[repr(C)]
struct Space {
    _private: [u8; 0],
}

#[repr(C)]
pub struct Index {
    _private: [u8; 0],
}

#[repr(C, align(16))]
pub struct Iterator {
    data: [u8; ITERATOR_SIZE],
}

impl Default for Iterator {
    fn default() -> Self {
        Self {
            data: [0; ITERATOR_SIZE],
        }
    }
}

type NewFn = unsafe extern "C" fn(*const c_char) -> *mut ReadView;
type DeleteFn = unsafe extern "C" fn(*mut ReadView);
type SpaceByIdFn = unsafe extern "C" fn(*const ReadView, u32) -> *mut Space;
type IndexByIdFn = unsafe extern "C" fn(*const Space, u32) -> *mut Index;
type GetFn = unsafe extern "C" fn(
    *const Index,
    *const c_char,
    *const c_char,
    *mut *const c_char,
    *mut u32,
) -> c_int;
type IteratorCreateFn =
    unsafe extern "C" fn(*mut Iterator, *const Index, c_int, *const c_char, *const c_char) -> c_int;
type IteratorDestroyFn = unsafe extern "C" fn(*mut Iterator);
type IteratorNextFn = unsafe extern "C" fn(*mut Iterator, *mut *const c_char, *mut u32) -> c_int;
type CountFn = unsafe extern "C" fn(*const Index, c_int, *const c_char, *const c_char) -> isize;

pub struct Api {
    new: NewFn,
    delete: DeleteFn,
    space_by_id: SpaceByIdFn,
    index_by_id: IndexByIdFn,
    get: GetFn,
    iterator_create: IteratorCreateFn,
    iterator_destroy: IteratorDestroyFn,
    iterator_next: IteratorNextFn,
    count: CountFn,
}

static API: OnceLock<Result<Api, String>> = OnceLock::new();

fn symbol(name: &CStr) -> *mut c_void {
    unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) }
}

macro_rules! load_symbol {
    ($name:literal, $ty:ty) => {{
        let name = CStr::from_bytes_with_nul(concat!($name, "\0").as_bytes()).unwrap();
        let symbol = symbol(name);
        if symbol.is_null() {
            return Err(format!(
                "Tarantool Enterprise API symbol `{}` is unavailable",
                name.to_string_lossy()
            ));
        }
        std::mem::transmute::<*mut c_void, $ty>(symbol)
    }};
}

impl Api {
    unsafe fn load() -> Result<Self, String> {
        Ok(Self {
            new: load_symbol!("box_raw_read_view_new", NewFn),
            delete: load_symbol!("box_raw_read_view_delete", DeleteFn),
            space_by_id: load_symbol!("box_raw_read_view_space_by_id", SpaceByIdFn),
            index_by_id: load_symbol!("box_raw_read_view_index_by_id", IndexByIdFn),
            get: load_symbol!("box_raw_read_view_get", GetFn),
            iterator_create: load_symbol!("box_raw_read_view_iterator_create", IteratorCreateFn),
            iterator_destroy: load_symbol!("box_raw_read_view_iterator_destroy", IteratorDestroyFn),
            iterator_next: load_symbol!("box_raw_read_view_iterator_next", IteratorNextFn),
            count: load_symbol!("box_raw_read_view_count", CountFn),
        })
    }

    pub unsafe fn open(&self, name: &CStr) -> *mut ReadView {
        (self.new)(name.as_ptr())
    }

    pub unsafe fn delete(&self, view: *mut ReadView) {
        (self.delete)(view);
    }

    pub unsafe fn index_by_id(
        &self,
        view: *const ReadView,
        space_id: u32,
        index_id: u32,
    ) -> *mut Index {
        let space = (self.space_by_id)(view, space_id);
        if space.is_null() {
            return std::ptr::null_mut();
        }
        (self.index_by_id)(space, index_id)
    }

    pub unsafe fn get(&self, index: *const Index, key: &[u8]) -> (c_int, *const u8, u32) {
        let mut data = std::ptr::null();
        let mut size = 0;
        let rc = (self.get)(
            index,
            key.as_ptr().cast(),
            key.as_ptr().add(key.len()).cast(),
            &mut data,
            &mut size,
        );
        (rc, data.cast(), size)
    }

    pub unsafe fn iterator_create(
        &self,
        iterator: &mut Iterator,
        index: *const Index,
        mode: c_int,
        key: &[u8],
    ) -> c_int {
        (self.iterator_create)(
            iterator,
            index,
            mode,
            key.as_ptr().cast(),
            key.as_ptr().add(key.len()).cast(),
        )
    }

    pub unsafe fn iterator_destroy(&self, iterator: &mut Iterator) {
        (self.iterator_destroy)(iterator);
    }

    pub unsafe fn iterator_next(&self, iterator: &mut Iterator) -> (c_int, *const u8, u32) {
        let mut data = std::ptr::null();
        let mut size = 0;
        let rc = (self.iterator_next)(iterator, &mut data, &mut size);
        (rc, data.cast(), size)
    }

    pub unsafe fn count(&self, index: *const Index, mode: c_int, key: &[u8]) -> isize {
        (self.count)(
            index,
            mode,
            key.as_ptr().cast(),
            key.as_ptr().add(key.len()).cast(),
        )
    }
}

pub fn is_available() -> bool {
    !symbol(ENTERPRISE_MARKER).is_null()
}

pub fn api() -> Result<&'static Api, &'static str> {
    API.get_or_init(|| unsafe { Api::load() })
        .as_ref()
        .map_err(String::as_str)
}
