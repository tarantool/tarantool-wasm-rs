//! Client ALPN defaults and a retained, live server preference list.
use crate::bindings::ssl_bio::TlsError;
use crate::tls_bio::{failure, invalid, BUFFER_LIMIT};
use foreign_types::ForeignTypeRef;
use openssl::ssl::{AlpnError, SslContextBuilder, SslContextRef};
use std::sync::{Arc, RwLock};

pub struct Policy(Arc<RwLock<Vec<u8>>>);

fn encode(protocols: Vec<String>) -> Result<Vec<u8>, TlsError> {
    let mut wire = Vec::new();
    for name in protocols {
        if name.is_empty() || name.len() > 255 || !name.is_ascii() {
            return Err(invalid("ALPN protocols must contain 1..255 ASCII bytes"));
        }
        if wire.len() + 1 + name.len() > BUFFER_LIMIT {
            return Err(invalid("ALPN protocol list limit exceeded"));
        }
        wire.push(name.len() as u8);
        wire.extend_from_slice(name.as_bytes());
    }
    Ok(wire)
}

fn select<'a>(preferred: &[u8], offered: &'a [u8]) -> Result<&'a [u8], AlpnError> {
    let mut preferred = preferred;
    while let Some((&size, rest)) = preferred.split_first() {
        if rest.len() < size as usize || size == 0 {
            return Err(AlpnError::ALERT_FATAL);
        }
        let (name, tail) = rest.split_at(size as usize);
        let mut candidates = offered;
        while let Some((&size, rest)) = candidates.split_first() {
            if rest.len() < size as usize || size == 0 {
                return Err(AlpnError::ALERT_FATAL);
            }
            let (candidate, tail) = rest.split_at(size as usize);
            if candidate == name {
                return Ok(candidate); // borrowed from OpenSSL's offered buffer
            }
            candidates = tail;
        }
        preferred = tail;
    }
    Err(AlpnError::NOACK)
}

pub fn install(
    builder: &mut SslContextBuilder,
    protocols: Vec<String>,
) -> Result<Policy, TlsError> {
    let wire = encode(protocols)?;
    builder.set_alpn_protos(&wire).map_err(failure)?;
    let shared = Arc::new(RwLock::new(wire));
    let callback = shared.clone();
    // Install even with an empty list: later updates enable negotiation. The
    // native context owns this callback/Arc as long as any SSL retains it.
    builder.set_alpn_select_callback(move |_, offered| {
        let preferred = callback.read().unwrap_or_else(|p| p.into_inner());
        select(&preferred, offered)
    });
    Ok(Policy(shared))
}

impl Policy {
    /// Update the client defaults and live server preference on the same ctx.
    ///
    /// # Safety
    /// Caller must exclude all associated SSL/SSL_CTX operations.
    pub unsafe fn update(
        &self,
        context: &SslContextRef,
        protocols: Vec<String>,
    ) -> Result<(), TlsError> {
        let wire = encode(protocols)?;
        // SAFETY: forwarded caller serialization contract.
        unsafe { self.update_wire(context, wire) }
    }

    /// Accept CPython's private binary ALPN format without UTF-8 conversion.
    ///
    /// # Safety
    /// Caller must exclude all associated SSL/SSL_CTX operations.
    pub unsafe fn update_wire(
        &self,
        context: &SslContextRef,
        wire: Vec<u8>,
    ) -> Result<(), TlsError> {
        if wire.len() > BUFFER_LIMIT {
            return Err(invalid("ALPN protocol list limit exceeded"));
        }
        let mut remaining = wire.as_slice();
        while let Some((&size, rest)) = remaining.split_first() {
            if size == 0 || rest.len() < size as usize {
                return Err(invalid("invalid ALPN protocol list"));
            }
            remaining = &rest[size as usize..];
        }
        let mut previous = self.0.write().unwrap_or_else(|p| p.into_inner());
        // SAFETY: public setter copies the bounded wire buffer synchronously;
        // serialized by the caller, and no borrowed pointer escapes this call.
        unsafe {
            if openssl_sys::SSL_CTX_set_alpn_protos(
                context.as_ptr(),
                wire.as_ptr(),
                wire.len() as u32,
            ) != 0
            {
                let error = openssl::error::ErrorStack::get();
                // Preserve prior defaults on older native implementations
                // that may clear their list before a failed allocation.
                openssl_sys::SSL_CTX_set_alpn_protos(
                    context.as_ptr(),
                    previous.as_ptr(),
                    previous.len() as u32,
                );
                return Err(failure(error));
            }
        }
        *previous = wire;
        Ok(())
    }
}
