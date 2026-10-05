//! Protocol diagnostics are copied into SSL-owned bounded memory. The native
//! callback never enters Wasm; guest callbacks run after each TLS operation.
use crate::bindings::ssl_bio::{TlsError, TlsMessage};
use crate::tls_bio::{failure, invalid, BUFFER_LIMIT};
use foreign_types::ForeignTypeRef;
use openssl::{
    error::ErrorStack,
    ex_data::Index,
    ssl::{Ssl, SslRef},
};
use std::{
    ffi::{c_int, c_void},
    sync::{Arc, Mutex, OnceLock},
};

type Callback = unsafe extern "C" fn(
    c_int,
    c_int,
    c_int,
    *const c_void,
    usize,
    *mut openssl_sys::SSL,
    *mut c_void,
);
unsafe extern "C" {
    // Public OpenSSL API, available since 0.9.7; no internal SSL layout.
    fn SSL_set_msg_callback(ssl: *mut openssl_sys::SSL, callback: Option<Callback>);
}

#[derive(Default)]
struct State {
    enabled: bool,
    messages: Vec<TlsMessage>,
    bytes: usize,
    overflow: bool,
}
impl State {
    fn reserve(&mut self, len: usize) -> bool {
        if !self.enabled || self.overflow {
            return false;
        }
        let size = len.saturating_add(std::mem::size_of::<TlsMessage>());
        if size > BUFFER_LIMIT - self.bytes || self.messages.len() >= 4096 {
            self.overflow = true;
            return false;
        }
        self.bytes += size;
        true
    }
}
type Shared = Arc<Mutex<State>>;
static INDEX: OnceLock<Result<Index<Ssl, Shared>, ErrorStack>> = OnceLock::new();
fn index() -> Result<Index<Ssl, Shared>, ErrorStack> {
    INDEX.get_or_init(Ssl::new_ex_index::<Shared>).clone()
}
fn shared(ssl: &SslRef) -> Result<Shared, TlsError> {
    ssl.ex_data(index().map_err(failure)?)
        .cloned()
        .ok_or_else(|| invalid("TLS message state unavailable"))
}

unsafe extern "C" fn callback(
    write: c_int,
    version: c_int,
    content_type: c_int,
    data: *const c_void,
    len: usize,
    ssl: *mut openssl_sys::SSL,
    _: *mut c_void,
) {
    let Some(Ok(index)) = INDEX.get() else {
        return;
    };
    if ssl.is_null() {
        return;
    }
    // SAFETY: OpenSSL supplies its live SSL and a buffer valid for this callback.
    // Only its already-attached ex_data is read; no native state is mutated.
    let ssl = unsafe { SslRef::from_ptr(ssl) };
    let Some(state) = ssl.ex_data(*index) else {
        return;
    };
    let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
    if !state.reserve(len) {
        return;
    }
    if data.is_null() && len != 0 {
        state.overflow = true;
        return;
    }
    let data = if len == 0 {
        Vec::new()
    } else {
        // SAFETY: length was bounded before touching the callback buffer.
        unsafe { std::slice::from_raw_parts(data.cast::<u8>(), len) }.to_vec()
    };
    state.messages.push(TlsMessage {
        write: write != 0,
        version,
        content_type,
        data,
    });
}

pub fn attach(ssl: &mut SslRef) -> Result<(), TlsError> {
    ssl.set_ex_data(
        index().map_err(failure)?,
        Arc::new(Mutex::new(State::default())),
    );
    Ok(())
}
pub fn enable(ssl: &SslRef, enabled: bool) -> Result<(), TlsError> {
    let state = shared(ssl)?;
    let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
    state.enabled = enabled;
    // SAFETY: operations on this SSL are serialized by BioConnection's host
    // contract. Record layers copy the native callback when initialized;
    // installing it only on opt-in preserves those OpenSSL copy semantics.
    unsafe {
        SSL_set_msg_callback(ssl.as_ptr(), enabled.then_some(callback));
    }
    if !enabled {
        state.messages.clear();
        state.bytes = 0;
        state.overflow = false;
    }
    Ok(())
}
pub fn take(ssl: &SslRef) -> Result<Vec<TlsMessage>, TlsError> {
    let state = shared(ssl)?;
    let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
    let messages = std::mem::take(&mut state.messages);
    state.bytes = 0;
    if std::mem::take(&mut state.overflow) {
        return Err(failure("TLS message queue exceeds 1 MiB or 4096 events"));
    }
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_queue_bounds_metadata_and_payload_before_copying() {
        let mut state = State::default();
        assert!(!state.reserve(1));
        state.enabled = true;
        assert!(state.reserve(BUFFER_LIMIT - std::mem::size_of::<TlsMessage>()));
        assert!(!state.reserve(0));
        assert!(state.overflow);
        state.bytes = 0;
        assert!(!state.reserve(0));
        state.overflow = false;
        assert!(!state.reserve(usize::MAX));
        assert!(state.overflow);
        state = State {
            enabled: true,
            ..State::default()
        };
        for _ in 0..4096 {
            assert!(state.reserve(0));
            state.messages.push(TlsMessage {
                write: false,
                version: 0,
                content_type: 0,
                data: Vec::new(),
            });
        }
        assert!(!state.reserve(0));
        assert!(state.overflow);
    }
    #[test]
    fn callback_rejects_oversized_buffer_before_dereference_and_recovers() {
        use openssl::ssl::{SslContextBuilder, SslMethod};
        let context = SslContextBuilder::new(SslMethod::tls()).unwrap().build();
        let mut ssl = Ssl::new(&context).unwrap();
        attach(&mut ssl).unwrap();
        enable(&ssl, true).unwrap();
        drop(context);
        // SAFETY: a live SSL with attached state; the deliberately NULL buffer
        // must never be read because its length exceeds the diagnostic budget.
        unsafe {
            callback(
                0,
                0,
                22,
                std::ptr::null(),
                BUFFER_LIMIT + 1,
                ssl.as_ptr(),
                std::ptr::null_mut(),
            );
        }
        assert!(take(&ssl).is_err());
        assert!(take(&ssl).unwrap().is_empty());
        let data = [1_u8, 0, 0, 0];
        // SAFETY: SSL and this fixed input remain valid throughout the call.
        unsafe {
            callback(
                1,
                0x303,
                22,
                data.as_ptr().cast(),
                data.len(),
                ssl.as_ptr(),
                std::ptr::null_mut(),
            );
        }
        assert_eq!(take(&ssl).unwrap()[0].data, data);
        enable(&ssl, false).unwrap();
        assert!(take(&ssl).unwrap().is_empty());
    }
}
