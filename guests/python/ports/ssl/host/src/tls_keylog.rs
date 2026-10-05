//! OpenSSL secret callbacks append only to connection-owned bounded memory.
//! Guest code drains between TLS calls and writes through its own capabilities.
use crate::bindings::ssl_bio::TlsError;
use crate::tls_bio::{failure, invalid, BUFFER_LIMIT};
use openssl::{
    error::ErrorStack,
    ex_data::Index,
    ssl::{Ssl, SslContextBuilder, SslRef},
};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Default)]
struct State {
    enabled: bool,
    bytes: Vec<u8>,
    overflow: bool,
}
impl State {
    fn append(&mut self, line: &str) {
        if !self.enabled || self.overflow {
            return;
        }
        if line.len().saturating_add(1) > BUFFER_LIMIT - self.bytes.len() {
            self.overflow = true;
            return;
        }
        self.bytes.extend_from_slice(line.as_bytes());
        self.bytes.push(b'\n');
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
        .ok_or_else(|| invalid("TLS key log state unavailable"))
}

pub fn install(builder: &mut SslContextBuilder) -> Result<(), TlsError> {
    let index = index().map_err(failure)?;
    builder.set_keylog_callback(move |ssl, line| {
        // This callback cannot call guest code or report errors to OpenSSL.
        // A sticky overflow is reported by take() after the TLS operation.
        if let Some(state) = ssl.ex_data(index) {
            state.lock().unwrap_or_else(|p| p.into_inner()).append(line);
        }
    });
    Ok(())
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
    if !enabled {
        state.bytes.clear();
        state.overflow = false;
    }
    Ok(())
}

pub fn take(ssl: &SslRef) -> Result<Vec<u8>, TlsError> {
    let state = shared(ssl)?;
    let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
    let bytes = std::mem::take(&mut state.bytes);
    if std::mem::take(&mut state.overflow) {
        return Err(failure("TLS key log queue exceeds 1 MiB"));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queue_is_disabled_by_default_and_overflow_is_sticky() {
        let mut state = State::default();
        state.append("ignored");
        assert!(state.bytes.is_empty());
        state.enabled = true;
        state.append(&"a".repeat(BUFFER_LIMIT - 1));
        assert_eq!(state.bytes.len(), BUFFER_LIMIT);
        state.append("overflow");
        assert!(state.overflow);
        state.bytes.clear();
        state.append("still ignored");
        assert!(state.bytes.is_empty());
    }
}
