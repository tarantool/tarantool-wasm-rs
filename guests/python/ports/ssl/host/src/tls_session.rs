//! Refcounted session ownership with a retained, exact originating SSL_CTX.
use crate::bindings::ssl_bio::{SessionInfo, SessionStats, TlsError};
use crate::tls_bio::{failure, invalid};
use foreign_types::ForeignTypeRef;
use openssl::ssl::{SslContext, SslContextRef, SslRef, SslSession};
use openssl_sys as ffi;

unsafe extern "C" {
    fn SSL_SESSION_has_ticket(session: *const ffi::SSL_SESSION) -> libc::c_int;
    fn SSL_SESSION_get_ticket_lifetime_hint(session: *const ffi::SSL_SESSION) -> libc::c_ulong;
}

#[derive(Clone)]
pub struct BioSession {
    inner: SslSession,
    context: SslContext,
}

impl BioSession {
    pub fn capture(ssl: &SslRef) -> Option<Self> {
        ssl.session().map(|session| Self {
            inner: session.to_owned(),
            context: ssl.ssl_context().to_owned(),
        })
    }

    pub fn info(&self) -> SessionInfo {
        // SAFETY: the session is refcounted and remains alive for both calls.
        SessionInfo {
            id: self.inner.id().to_vec(),
            time: self.inner.time(),
            timeout: self.inner.timeout(),
            has_ticket: unsafe { SSL_SESSION_has_ticket(self.inner.as_ptr()) } != 0,
            ticket_lifetime_hint: unsafe {
                SSL_SESSION_get_ticket_lifetime_hint(self.inner.as_ptr())
            } as u64,
        }
    }

    pub fn apply(&self, ssl: &mut SslRef) -> Result<(), TlsError> {
        if ssl.is_server() {
            return Err(invalid("cannot set session on a server connection"));
        }
        if ssl.is_init_finished() {
            return Err(invalid("cannot set session after handshake"));
        }
        if ssl.ssl_context().as_ptr() != self.context.as_ptr() {
            return Err(invalid("session refers to a different SSLContext snapshot"));
        }
        // SAFETY: pointer identity above proves this session came from the
        // exact same retained SSL_CTX. The resource-table operation serializes
        // SSL mutation; SSL_set_session acquires its own session reference.
        unsafe { ssl.set_session(&self.inner) }.map_err(failure)
    }
}

pub fn stats(context: &SslContextRef) -> SessionStats {
    // Public OpenSSL SSL_CTX_sess_* macros delegate to these SSL_CTX_ctrl
    // numbers (20..31) with no pointer argument. Values are signed long counts.
    let count = |command| unsafe {
        ffi::SSL_CTX_ctrl(context.as_ptr(), command, 0, std::ptr::null_mut()) as i64
    };
    SessionStats {
        number: count(20),
        connect: count(21),
        connect_good: count(22),
        connect_renegotiate: count(23),
        accept: count(24),
        accept_good: count(25),
        accept_renegotiate: count(26),
        hits: count(27),
        misses: count(29),
        timeouts: count(30),
        cache_full: count(31),
    }
}
