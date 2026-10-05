//! Guest-owned DH parameters and an explicit OpenSSL named group.
use crate::bindings::ssl_bio::{
    ContextPolicy, KeyExchange, KeyExchangeUpdate, SeededPolicy, TlsError,
};
use crate::tls_bio::{failure, invalid, BUFFER_LIMIT};
use foreign_types::ForeignTypeRef;
use openssl::{
    error::ErrorStack,
    ssl::{SslContextBuilder, SslContextRef, SslMethod},
};
use openssl_sys as ffi;
use std::ffi::CString;

pub fn apply(builder: &mut SslContextBuilder, settings: &KeyExchange) -> Result<(), TlsError> {
    if let Some(name) = &settings.curve {
        let mut nid = curve_nid(name)?;
        // SAFETY: SSL_CTX_set1_groups is the public SSL_CTX_ctrl(91) macro.
        // The exclusively owned builder is live; OpenSSL copies one valid
        // int NID synchronously and does not retain this stack pointer.
        let result = unsafe {
            ffi::SSL_CTX_ctrl(
                builder.as_ptr(),
                91,
                1,
                (&mut nid as *mut libc::c_int).cast(),
            )
        };
        if result != 1 {
            return Err(failure(ErrorStack::get()));
        }
    }
    if let Some(pem) = &settings.dh_pem {
        if pem.len() > BUFFER_LIMIT {
            return Err(invalid("DH parameter limit exceeded"));
        }
        let params = crate::tls_pem::dh(pem)?;
        builder.set_tmp_dh(&params).map_err(failure)?;
    }
    Ok(())
}

fn curve_nid(name: &str) -> Result<libc::c_int, TlsError> {
    if name.len() > 256 {
        return Err(invalid("unknown elliptic curve name"));
    }
    let name = CString::new(name.as_bytes()).map_err(|_| invalid("embedded null byte"))?;
    // SAFETY: CString remains live throughout the public name lookup.
    let nid = unsafe { ffi::OBJ_sn2nid(name.as_ptr()) };
    if nid == 0 {
        return Err(invalid("unknown elliptic curve name"));
    }
    Ok(nid)
}

/// Change one native parameter, preserving all other context state.
///
/// # Safety
/// Caller must exclude every associated SSL/SSL_CTX operation.
pub unsafe fn update_shared(
    context: &SslContextRef,
    update: KeyExchangeUpdate,
) -> Result<(), TlsError> {
    let settings = match update {
        KeyExchangeUpdate::DhPem(pem) => KeyExchange {
            dh_pem: Some(pem),
            curve: None,
        },
        KeyExchangeUpdate::Curve(name) => KeyExchange {
            dh_pem: None,
            curve: Some(name),
        },
    };
    // Validate only the changed parameter, using the live effective level.
    let mut validation = SslContextBuilder::new(SslMethod::tls()).map_err(failure)?;
    validation.set_security_level(context.security_level());
    apply(&mut validation, &settings)?;
    if let Some(name) = &settings.curve {
        let mut nid = curve_nid(name)?;
        // SAFETY: public SSL_CTX_set1_groups macro copies one validated NID;
        // the caller serializes operations and no stack pointer is retained.
        if unsafe {
            ffi::SSL_CTX_ctrl(
                context.as_ptr(),
                91,
                1,
                (&mut nid as *mut libc::c_int).cast(),
            )
        } != 1
        {
            return Err(failure(ErrorStack::get()));
        }
    }
    if let Some(pem) = &settings.dh_pem {
        let params = crate::tls_pem::dh(pem)?;
        // SAFETY: public setter retains/copies parameters; the owning DH
        // value lives through the call and may be dropped after it returns.
        if unsafe { ffi::SSL_CTX_set_tmp_dh(context.as_ptr(), params.as_ptr()) } != 1 {
            return Err(failure(ErrorStack::get()));
        }
    }
    Ok(())
}

pub fn prepare(settings: KeyExchange, policy: ContextPolicy) -> Result<KeyExchange, TlsError> {
    let mut builder = SslContextBuilder::new(SslMethod::tls()).map_err(failure)?;
    crate::tls_policy::apply(&mut builder, &policy)?;
    finish_preparation(settings, builder)
}

pub fn prepare_seeded(
    settings: KeyExchange,
    policy: SeededPolicy,
) -> Result<KeyExchange, TlsError> {
    let mut builder = SslContextBuilder::new(SslMethod::tls()).map_err(failure)?;
    crate::tls_policy::seed(&mut builder, &policy.seed)?;
    crate::tls_policy::apply(&mut builder, &policy.policy)?;
    finish_preparation(settings, builder)
}

fn finish_preparation(
    mut settings: KeyExchange,
    mut builder: SslContextBuilder,
) -> Result<KeyExchange, TlsError> {
    apply(&mut builder, &settings)?;
    if let Some(pem) = &settings.dh_pem {
        settings.dh_pem = Some(crate::tls_pem::dh(pem)?.params_to_pem().map_err(failure)?);
    }
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::ssl_bio::ErrorKind;

    fn policy(ciphers: Option<&str>) -> ContextPolicy {
        ContextPolicy {
            options: 0,
            verify_flags: 0,
            ciphers: ciphers.map(str::to_owned),
            hostname_common_name: true,
        }
    }

    #[test]
    fn validates_dh_sizes_and_current_security_level() {
        let strong = include_bytes!("../../../../tests/fixtures/ssl/dh2048.pem").to_vec();
        let weak = include_bytes!("../../../../tests/fixtures/ssl/dh1024.pem").to_vec();
        let settings = |pem| KeyExchange {
            dh_pem: Some(pem),
            curve: None,
        };
        assert!(prepare(settings(strong), policy(None)).is_ok());
        assert!(matches!(
            prepare(settings(weak.clone()), policy(None))
                .unwrap_err()
                .kind,
            ErrorKind::Failure
        ));
        assert!(prepare(settings(weak), policy(Some("DEFAULT:@SECLEVEL=0"))).is_ok());
        assert!(matches!(
            prepare(settings(vec![0; BUFFER_LIMIT + 1]), policy(None))
                .unwrap_err()
                .kind,
            ErrorKind::InvalidInput
        ));
        assert!(prepare(settings(b"not DH parameters".to_vec()), policy(None)).is_err());
    }

    #[test]
    fn validates_exact_openssl_short_group_names() {
        for name in ["prime256v1", "secp384r1", "X25519"] {
            assert!(prepare(
                KeyExchange {
                    dh_pem: None,
                    curve: Some(name.into())
                },
                policy(None)
            )
            .is_ok());
        }
        for name in ["P-256", "prime256v1:secp384r1", "invalid", "prime256v1\0x"] {
            assert!(matches!(
                prepare(
                    KeyExchange {
                        dh_pem: None,
                        curve: Some(name.into())
                    },
                    policy(None)
                )
                .unwrap_err()
                .kind,
                ErrorKind::InvalidInput
            ));
        }
        assert!(matches!(
            prepare(
                KeyExchange {
                    dh_pem: None,
                    curve: Some("RSA".into())
                },
                policy(None)
            )
            .unwrap_err()
            .kind,
            ErrorKind::Failure
        ));
    }
}
