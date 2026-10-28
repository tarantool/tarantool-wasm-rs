//! OpenSSL is optional at runtime; failed loading is cached for this process.
use crate::{bindings::ssl_bio::TlsError, tls_bio::failure};
use std::ffi::CStr;

unsafe extern "C" {
    fn twr_python_ssl_error() -> *const libc::c_char;
}

pub(crate) fn ensure_available() -> Result<(), TlsError> {
    // SAFETY: the loader synchronizes initialization and retains its immutable
    // diagnostic string and successfully loaded libraries for process lifetime.
    let error = unsafe { twr_python_ssl_error() };
    if error.is_null() {
        Ok(())
    } else {
        Err(failure(unsafe { CStr::from_ptr(error) }.to_string_lossy()))
    }
}
