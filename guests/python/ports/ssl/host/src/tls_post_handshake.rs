//! TLS 1.3 authentication uses the existing in-memory TLS transport only.
use crate::bindings::ssl_bio::{Tls13Settings, TlsError};
use crate::tls_bio::{failure, invalid};
use foreign_types::ForeignTypeRef;
use openssl::{
    error::ErrorStack,
    ssl::{SslContextBuilder, SslContextRef, SslRef, SslVerifyMode},
};
use openssl_sys as ffi;

extern "C" {
    fn SSL_verify_client_post_handshake(ssl: *mut ffi::SSL) -> libc::c_int;
    fn SSL_set_post_handshake_auth(ssl: *mut ffi::SSL, enabled: libc::c_int);
}

pub fn configure(
    builder: &mut SslContextBuilder,
    server: bool,
    settings: &Tls13Settings,
) -> Result<(), TlsError> {
    if settings.num_tickets > i64::MAX as u64 {
        return Err(invalid("num_tickets exceeds signed long"));
    }
    let count = usize::try_from(settings.num_tickets)
        .map_err(|_| invalid("num_tickets exceeds host size_t"))?;
    builder.set_num_tickets(count).map_err(failure)?;
    // SAFETY: builder owns the live SSL_CTX. This only controls the client's
    // ClientHello extension; server authentication is a verification flag.
    unsafe {
        ffi::SSL_CTX_set_post_handshake_auth(
            builder.as_ptr(),
            i32::from(!server && settings.post_handshake_auth),
        );
    }
    if server && settings.post_handshake_auth {
        // SAFETY: builder owns the SSL_CTX and this getter only reads flags.
        let mode = SslVerifyMode::from_bits_retain(unsafe {
            ffi::SSL_CTX_get_verify_mode(builder.as_ptr())
        });
        if mode.contains(SslVerifyMode::PEER) {
            builder.set_verify(mode | SslVerifyMode::POST_HANDSHAKE);
        }
    }
    Ok(())
}

/// Update the defaults copied by SSL_new, preserving native context identity.
///
/// # Safety
/// Caller must serialize this with all associated SSL/SSL_CTX operations.
pub unsafe fn update_shared(
    context: &SslContextRef,
    server: bool,
    settings: &Tls13Settings,
) -> Result<(), TlsError> {
    if settings.num_tickets > i64::MAX as u64 {
        return Err(invalid("num_tickets exceeds signed long"));
    }
    let count = usize::try_from(settings.num_tickets)
        .map_err(|_| invalid("num_tickets exceeds host size_t"))?;
    let pointer = context.as_ptr();
    let mut mode = context.verify_mode();
    mode.remove(SslVerifyMode::POST_HANDSHAKE);
    if server && settings.post_handshake_auth && mode.contains(SslVerifyMode::PEER) {
        mode |= SslVerifyMode::POST_HANDSHAKE;
    }
    // SAFETY: caller excludes associated operations. Public setters update
    // native defaults only; previous SSL objects retain their copied values.
    unsafe {
        if ffi::SSL_CTX_set_num_tickets(pointer, count) != 1 {
            return Err(failure(ErrorStack::get()));
        }
        ffi::SSL_CTX_set_post_handshake_auth(
            pointer,
            i32::from(!server && settings.post_handshake_auth),
        );
        ffi::SSL_CTX_set_verify(pointer, mode.bits(), None);
    }
    Ok(())
}

/// Apply role-specific PHA to a newly created, exclusively owned SSL object.
pub fn configure_connection(ssl: &mut SslRef, server: bool, enabled: bool) {
    let mut mode = ssl.verify_mode();
    mode.remove(SslVerifyMode::POST_HANDSHAKE);
    if server && enabled && mode.contains(SslVerifyMode::PEER) {
        mode |= SslVerifyMode::POST_HANDSHAKE;
    }
    ssl.set_verify(mode);
    // SAFETY: the SSL is unpublished and exclusively borrowed. PHA client
    // advertisement and server verification are selected per actual role.
    unsafe { SSL_set_post_handshake_auth(ssl.as_ptr(), i32::from(!server && enabled)) };
}

pub fn request(ssl: &mut SslRef) -> Result<(), TlsError> {
    // SAFETY: the caller exclusively owns the live SSL. OpenSSL validates
    // role/version/negotiated extension and queues a request without I/O.
    if unsafe { SSL_verify_client_post_handshake(ssl.as_ptr()) } == 1 {
        Ok(())
    } else {
        Err(failure(ErrorStack::get()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openssl::ssl::SslMethod;
    #[test]
    fn rejects_unsigned_ticket_count_outside_python_range() {
        let mut builder = SslContextBuilder::new(SslMethod::tls()).unwrap();
        assert!(configure(
            &mut builder,
            true,
            &Tls13Settings {
                post_handshake_auth: true,
                num_tickets: u64::MAX,
            }
        )
        .is_err());
        assert_eq!(builder.build().num_tickets(), 2);
    }
    #[test]
    fn live_tls13_defaults_are_copied_and_invalid_updates_are_atomic() {
        use openssl::ssl::Ssl;
        let mut builder = SslContextBuilder::new(SslMethod::tls()).unwrap();
        builder.set_verify(SslVerifyMode::PEER);
        let context = builder.build();
        let before = Ssl::new(&context).unwrap();
        // SAFETY: context/SSL references remain local, with no concurrent calls.
        unsafe {
            update_shared(
                &context,
                true,
                &Tls13Settings {
                    post_handshake_auth: true,
                    num_tickets: 0,
                },
            )
        }
        .unwrap();
        let after = Ssl::new(&context).unwrap();
        assert_eq!(before.num_tickets(), 2);
        assert_eq!(before.verify_mode(), SslVerifyMode::PEER);
        assert_eq!(after.num_tickets(), 0);
        assert_eq!(
            after.verify_mode(),
            SslVerifyMode::PEER | SslVerifyMode::POST_HANDSHAKE
        );
        assert!(unsafe {
            update_shared(
                &context,
                true,
                &Tls13Settings {
                    post_handshake_auth: false,
                    num_tickets: u64::MAX,
                },
            )
        }
        .is_err());
        assert_eq!(context.num_tickets(), 0);
        assert!(context
            .verify_mode()
            .contains(SslVerifyMode::POST_HANDSHAKE));
        unsafe {
            update_shared(
                &context,
                true,
                &Tls13Settings {
                    post_handshake_auth: false,
                    num_tickets: 3,
                },
            )
        }
        .unwrap();
        assert_eq!(context.num_tickets(), 3);
        assert_eq!(context.verify_mode(), SslVerifyMode::PEER);
    }
}
