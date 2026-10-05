//! Decode the last error in openssl-rs' serialized stack without consulting or
//! consuming thread-local OpenSSL error state. Names use CPython 3.12's tables.
use crate::bindings::ssl_bio::ErrorDetails;
include!(concat!(env!("OUT_DIR"), "/ssl_errors.rs"));

pub fn verification_message(code: i32) -> String {
    openssl::init();
    // SAFETY: public API returns static literal strings for all codes, including
    // unknown values, in supported OpenSSL 1.1.1+. No queue or TLS state access.
    let message = unsafe { openssl_sys::X509_verify_cert_error_string(code.into()) };
    if message.is_null() {
        return "unknown certificate verification error".into();
    }
    // SAFETY: static NUL-terminated OpenSSL string copied before return.
    unsafe { std::ffi::CStr::from_ptr(message) }
        .to_string_lossy()
        .into_owned()
}

pub fn details(message: &str) -> ErrorDetails {
    let mut result = ErrorDetails {
        library: None,
        reason: None,
    };
    if message.len() > crate::tls_bio::BUFFER_LIMIT {
        return result;
    }
    let code = message
        .match_indices("error:")
        .filter_map(|(offset, _)| {
            if offset != 0 && !message[..offset].ends_with(", ") {
                return None;
            }
            let (hex, _) = message[offset + 6..].split_once(':')?;
            if hex.is_empty() || hex.len() > 16 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            u64::from_str_radix(hex, 16).ok()
        })
        .last();
    let Some(code) = code else {
        return result;
    };
    if code as u128 > std::os::raw::c_ulong::MAX as u128 {
        return result;
    }
    let code = code as std::os::raw::c_ulong;
    // Pure numeric extraction via public macros; no error queue or I/O.
    let (library, reason) = (
        openssl_sys::ERR_GET_LIB(code),
        openssl_sys::ERR_GET_REASON(code),
    );
    result.library = LIBRARIES
        .iter()
        .rev()
        .find(|(id, _)| *id == library)
        .map(|(_, name)| (*name).to_owned());
    result.reason = REASONS
        .iter()
        .rev()
        .find(|(lib, id, _)| *lib == library && *id == reason)
        .map(|(_, _, name)| (*name).to_owned());
    result
}
