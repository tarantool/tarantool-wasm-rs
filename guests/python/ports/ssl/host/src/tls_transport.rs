//! Supply BIO_CTRL_EOF missing from openssl-rs' stream BIO. OpenSSL can then
//! classify truncated input and apply SSL_OP_IGNORE_UNEXPECTED_EOF normally.
use libc::{c_char, c_int, c_long};
use openssl_sys as ffi;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

type Callback = unsafe extern "C" fn(
    *mut ffi::BIO,
    c_int,
    *const c_char,
    usize,
    c_int,
    c_long,
    c_int,
    *mut usize,
) -> c_long;
extern "C" {
    fn BIO_set_callback_ex(bio: *mut ffi::BIO, callback: Option<Callback>);
    fn BIO_set_callback_arg(bio: *mut ffi::BIO, arg: *mut c_char);
    fn BIO_get_callback_arg(bio: *const ffi::BIO) -> *mut c_char;
}

/// # Safety
/// The caller serializes all associated SSL/BIO operations and keeps an Arc
/// owner alive until after the BIO is destroyed. No callback existed on this
/// fresh openssl-rs stream BIO; its method and data remain unchanged.
pub unsafe fn attach(bio: *mut ffi::BIO, eof: &Arc<AtomicBool>) {
    // SAFETY: forwarded ownership/serialization contract. No ownership transfer.
    unsafe {
        BIO_set_callback_arg(bio, Arc::as_ptr(eof).cast_mut().cast());
        BIO_set_callback_ex(bio, Some(callback));
    }
}

unsafe extern "C" fn callback(
    bio: *mut ffi::BIO,
    operation: c_int,
    _arg: *const c_char,
    _length: usize,
    command: c_int,
    _number: c_long,
    result: c_int,
    _processed: *mut usize,
) -> c_long {
    // BIO_CB_CTRL | BIO_CB_RETURN. All other operations, including destruction,
    // preserve the original result and never dereference callback state.
    if operation == (0x06 | 0x80) && command == ffi::BIO_CTRL_EOF {
        // SAFETY: attach's owner lives through BIO destruction. Atomic state
        // avoids aliasing transport's Read borrow; this callback cannot unwind.
        let state = unsafe { BIO_get_callback_arg(bio).cast::<AtomicBool>().as_ref() };
        return state.is_some_and(|eof| eof.load(Ordering::Relaxed)) as c_long;
    }
    result as c_long
}
