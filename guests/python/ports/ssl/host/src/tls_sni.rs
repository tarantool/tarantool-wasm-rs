//! Suspend ClientHello processing so callbacks run in the guest between WIT calls.
use crate::bindings::ssl_bio::{SniRequest, TlsError};
use crate::tls_bio::{failure, invalid};
use foreign_types::ForeignTypeRef;
use openssl::{
    error::ErrorStack,
    ex_data::Index,
    ssl::{SniError, Ssl, SslContext, SslContextBuilder, SslRef},
};
use openssl_sys as ffi;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, OnceLock,
};

/// Retained context policy, consulted at each ClientHello (including HRR).
#[derive(Clone)]
pub struct Policy(Arc<AtomicBool>);
impl Policy {
    pub fn new(enabled: bool) -> Self {
        Self(Arc::new(AtomicBool::new(enabled)))
    }
    pub fn update(&self, enabled: bool) {
        self.0.store(enabled, Ordering::Relaxed);
    }
    fn enabled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[derive(Default)]
enum Phase {
    #[default]
    Idle,
    Pending(Option<String>),
    Resolved(Option<u8>),
}
pub struct State {
    current: Policy,
    initial: Policy,
    acknowledged: bool,
    phase: Phase,
    replacement: Option<(SslContext, Policy)>,
}
type Shared = Arc<Mutex<State>>;
static INDEX: OnceLock<Result<Index<Ssl, Shared>, ErrorStack>> = OnceLock::new();
fn index() -> Result<Index<Ssl, Shared>, ErrorStack> {
    INDEX.get_or_init(Ssl::new_ex_index::<Shared>).clone()
}

pub fn install(builder: &mut SslContextBuilder) -> Result<(), TlsError> {
    index().map_err(failure)?;
    // SAFETY: callback is static, carries no borrowed userdata, and operates
    // synchronously on each connection's own SSL ex-data.
    unsafe {
        ffi::SSL_CTX_set_client_hello_cb(
            builder.as_ptr(),
            Some(client_hello),
            std::ptr::null_mut(),
        );
    }
    // OpenSSL requires the later servername callback to acknowledge SNI on
    // the wire. Selection has already completed in the guest; no guest call
    // occurs here, including after switching to a context without a callback.
    builder.set_servername_callback(|ssl, _| {
        let state = shared(ssl).map_err(|_| SniError::ALERT_FATAL)?;
        let state = state.lock().unwrap_or_else(|poison| poison.into_inner());
        if state.acknowledged {
            Ok(())
        } else {
            Err(SniError::NOACK)
        }
    });
    Ok(())
}

pub fn attach(ssl: &mut SslRef, policy: &Policy) -> Result<(), TlsError> {
    ssl.set_ex_data(
        index().map_err(failure)?,
        Arc::new(Mutex::new(State {
            current: policy.clone(),
            initial: policy.clone(),
            acknowledged: false,
            phase: Phase::Idle,
            replacement: None,
        })),
    );
    Ok(())
}

fn shared(ssl: &SslRef) -> Result<Shared, TlsError> {
    ssl.ex_data(index().map_err(failure)?)
        .cloned()
        .ok_or_else(|| invalid("SNI state unavailable"))
}

pub fn pending(ssl: &SslRef) -> Result<Option<SniRequest>, TlsError> {
    let shared = shared(ssl)?;
    let state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
    Ok(match &state.phase {
        Phase::Pending(name) => Some(SniRequest {
            server_name: name.clone(),
        }),
        _ => None,
    })
}

pub fn resolve(ssl: &SslRef, alert: Option<u8>) -> Result<(), TlsError> {
    let shared = shared(ssl)?;
    let mut state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
    if !matches!(state.phase, Phase::Pending(_)) {
        return Err(invalid("no pending SNI callback"));
    }
    state.phase = Phase::Resolved(alert);
    Ok(())
}

pub fn change_context(
    ssl: &mut SslRef,
    context: &SslContext,
    policy: &Policy,
) -> Result<(), TlsError> {
    let shared = shared(ssl)?;
    let mut state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
    if matches!(state.phase, Phase::Pending(_) | Phase::Resolved(_)) {
        // Keep the original ClientHello callback installed until its resumed
        // invocation. A replacement context cannot swallow the pending alert.
        state.replacement = Some((context.clone(), policy.clone()));
        return Ok(());
    }
    ssl.set_ssl_context(context).map_err(failure)?;
    state.current = policy.clone();
    Ok(())
}

fn decode_name(bytes: &[u8]) -> Result<Option<String>, ()> {
    if bytes.len() < 5 {
        return Err(());
    }
    let list = u16::from_be_bytes([bytes[0], bytes[1]]) as usize;
    let size = u16::from_be_bytes([bytes[3], bytes[4]]) as usize;
    if list != bytes.len() - 2
        || bytes[2] != 0
        || size == 0
        || size > 255
        || size != bytes.len() - 5
    {
        return Err(());
    }
    let name = &bytes[5..];
    if !name.is_ascii() || name.contains(&0) {
        return Err(());
    }
    Ok(Some(std::str::from_utf8(name).map_err(|_| ())?.to_owned()))
}

unsafe extern "C" fn client_hello(
    pointer: *mut ffi::SSL,
    alert: *mut libc::c_int,
    _: *mut libc::c_void,
) -> libc::c_int {
    // SAFETY: OpenSSL supplies the live SSL under the exclusive handshake call
    // and a writable alert pointer. No guest code is invoked from this callback.
    let ssl = unsafe { SslRef::from_ptr_mut(pointer) };
    let Ok(shared) = shared(ssl) else {
        unsafe {
            *alert = 80;
        }
        return ffi::SSL_CLIENT_HELLO_ERROR;
    };
    let mut state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
    match &state.phase {
        Phase::Pending(_) => return ffi::SSL_CLIENT_HELLO_RETRY,
        Phase::Resolved(value) => {
            let rejection = *value;
            let replacement = state.replacement.take();
            if let Some((_, policy)) = &replacement {
                state.current = policy.clone();
            }
            state.acknowledged = rejection.is_none();
            state.phase = Phase::Idle;
            drop(state);
            if let Some((context, _)) = replacement {
                if ssl.set_ssl_context(&context).is_err() {
                    unsafe {
                        *alert = 80;
                    }
                    return ffi::SSL_CLIENT_HELLO_ERROR;
                }
            }
            if let Some(value) = rejection {
                unsafe {
                    *alert = libc::c_int::from(value);
                }
                return ffi::SSL_CLIENT_HELLO_ERROR;
            }
            return ffi::SSL_CLIENT_HELLO_SUCCESS;
        }
        Phase::Idle => {}
    }
    // A context callback can be changed after SSL_new or a pre-handshake
    // context switch. Keep both policies live, matching OpenSSL's fallback
    // from the current context to the original session context. Resolve an
    // already suspended callback above even if it disabled itself.
    if !state.current.enabled() && !state.initial.enabled() {
        state.acknowledged = false;
        return ffi::SSL_CLIENT_HELLO_SUCCESS;
    }
    let mut data = std::ptr::null();
    let mut size = 0;
    // SAFETY: the extension slice is borrowed only during this callback. Its
    // bounded length is checked before constructing a Rust slice.
    let present = unsafe { ffi::SSL_client_hello_get0_ext(pointer, 0, &mut data, &mut size) };
    let name = if present == 0 {
        Ok(None)
    } else if data.is_null() || size > 65535 {
        Err(())
    } else {
        decode_name(unsafe { std::slice::from_raw_parts(data, size) })
    };
    match name {
        Ok(name) => {
            state.phase = Phase::Pending(name);
            ffi::SSL_CLIENT_HELLO_RETRY
        }
        Err(()) => {
            unsafe {
                *alert = 50;
            }
            ffi::SSL_CLIENT_HELLO_ERROR
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_malformed_server_name_extension() {
        assert_eq!(
            decode_name(b"\0\x0c\0\0\x09localhost"),
            Ok(Some("localhost".into()))
        );
        for bytes in [
            b"".as_slice(),
            b"\0\0",
            b"\0\x0c\0\0\x08localhost",
            b"\0\x04\0\0\x01\0",
            b"\0\x04\0\0\x01\xff",
        ] {
            assert!(decode_name(bytes).is_err());
        }
    }
}
