//! Read-only TLS connection metadata; no transport or host capabilities.
use crate::bindings::ssl_bio::{SharedCipher, TlsError};
use crate::tls_bio::invalid;
use foreign_types::ForeignTypeRef;
use openssl::{
    ssl::{SslCipher, SslRef},
    stack::StackRef,
};
use openssl_sys as ffi;
use std::collections::HashSet;
use std::ffi::CStr;

unsafe extern "C" {
    fn SSL_get_ciphers(ssl: *const ffi::SSL) -> *mut ffi::stack_st_SSL_CIPHER;
    fn SSL_get_client_ciphers(ssl: *const ffi::SSL) -> *mut ffi::stack_st_SSL_CIPHER;
    fn SSL_CIPHER_get_id(cipher: *const ffi::SSL_CIPHER) -> libc::c_ulong;
    // Public functions remain available with OPENSSL_NO_COMP. Their opaque
    // COMP_METHOD pointer is only passed back to OpenSSL, never dereferenced.
    fn SSL_get_current_compression(ssl: *const ffi::SSL) -> *const libc::c_void;
    fn SSL_COMP_get_name(method: *const libc::c_void) -> *const libc::c_char;
}

pub fn compression(ssl: &SslRef) -> Result<Option<String>, TlsError> {
    // SAFETY: SSL remains live, and the returned method/name are borrowed from
    // OpenSSL. Copy the name synchronously; no native reference escapes.
    let method = unsafe { SSL_get_current_compression(ssl.as_ptr()) };
    if method.is_null() {
        return Ok(None);
    }
    let name = unsafe { SSL_COMP_get_name(method) };
    if name.is_null() {
        return Ok(None);
    }
    let name = unsafe { CStr::from_ptr(name) };
    if name.to_bytes().len() > 4096 {
        return Err(invalid("TLS compression name limit exceeded"));
    }
    Ok(Some(
        name.to_str()
            .map_err(|e| invalid(e.to_string()))?
            .to_owned(),
    ))
}

pub fn channel_binding(ssl: &SslRef) -> Result<Option<Vec<u8>>, TlsError> {
    // CPython 3.12 chooses the local Finished for a new client session or a
    // resumed server session, and the peer Finished otherwise. A zero length
    // means that Finished has not arrived yet. OpenSSL owns these bytes.
    let mut data = [0; 128];
    let length = if ssl.session_reused() ^ !ssl.is_server() {
        ssl.finished(&mut data)
    } else {
        ssl.peer_finished(&mut data)
    };
    if length > data.len() {
        return Err(invalid("TLS channel binding limit exceeded"));
    }
    Ok((length != 0).then(|| data[..length].to_vec()))
}

pub fn shared_ciphers(ssl: &SslRef) -> Result<Option<Vec<SharedCipher>>, TlsError> {
    // SAFETY: both stacks are borrowed from this live SSL/context. This
    // function neither mutates the SSL nor lets a stack reference escape.
    let server = unsafe { SSL_get_ciphers(ssl.as_ptr()) };
    let client = unsafe { SSL_get_client_ciphers(ssl.as_ptr()) };
    if server.is_null() || client.is_null() {
        return Ok(None);
    }
    let server = unsafe { StackRef::<SslCipher>::from_ptr(server) };
    let client = unsafe { StackRef::<SslCipher>::from_ptr(client) };
    if server.len() > 4096 || client.len() > 4096 {
        return Err(invalid("TLS shared cipher limit exceeded"));
    }
    // Match cipher IDs without sorting/mutating OpenSSL's borrowed stacks.
    let offered: HashSet<_> = client
        .iter()
        .map(|cipher| unsafe { SSL_CIPHER_get_id(cipher.as_ptr()) })
        .collect();
    let mut shared = Vec::new();
    for cipher in server {
        if offered.contains(&unsafe { SSL_CIPHER_get_id(cipher.as_ptr()) }) {
            shared.push(SharedCipher {
                name: cipher.name().to_owned(),
                protocol: cipher.version().to_owned(),
                strength_bits: cipher.bits().secret as u32,
            });
        }
    }
    Ok(Some(shared))
}
