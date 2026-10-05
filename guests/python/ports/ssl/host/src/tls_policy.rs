//! Python-compatible context policy and serialized updates to live contexts.
use crate::bindings::ssl_bio::{
    CipherDetails, ContextPolicy, PolicyDetails, PolicySeed, SeededPolicy, TlsError,
};
use crate::tls_bio::{failure, invalid, BUFFER_LIMIT};
use foreign_types::ForeignTypeRef;
use openssl::{
    nid::Nid,
    ssl::{Ssl, SslCipher, SslCipherRef, SslContextBuilder, SslContextRef, SslMethod, SslOptions},
    stack::StackRef,
    x509::verify::{X509CheckFlags, X509VerifyFlags},
};
use openssl_sys as ffi;
use std::ffi::{CStr, CString};

pub const DEFAULT_CIPHERS: &str =
    "@SECLEVEL=2:ECDH+AESGCM:ECDH+CHACHA20:ECDH+AES:DHE+AES:!aNULL:!eNULL:!aDSS:!SHA1:!AESCCM";

pub fn seed(builder: &mut SslContextBuilder, seed: &PolicySeed) -> Result<(), TlsError> {
    if seed.security_level > 5 || !matches!(seed.suiteb_flags, 0 | 0x10000 | 0x20000 | 0x30000) {
        return Err(invalid("invalid inherited cipher policy"));
    }
    builder.set_security_level(seed.security_level);
    // Public SSL_CTX_set_cert_flags macro, SSL_CTRL_CERT_FLAGS=99. This
    // uniquely owned builder has not escaped; Suite B flags persist across
    // later cipher-list calls. No cipher expression is altered or replayed.
    unsafe {
        ffi::SSL_CTX_ctrl(
            builder.as_ptr(),
            99,
            seed.suiteb_flags.into(),
            std::ptr::null_mut(),
        )
    };
    Ok(())
}

pub fn inspect_seeded(policy: SeededPolicy) -> Result<PolicyDetails, TlsError> {
    let mut builder = SslContextBuilder::new(SslMethod::tls()).map_err(failure)?;
    seed(&mut builder, &policy.seed)?;
    apply(&mut builder, &policy.policy)?;
    details(&builder.build())
}

pub fn host_flags(common_name: bool) -> X509CheckFlags {
    let mut flags = X509CheckFlags::NO_PARTIAL_WILDCARDS;
    if !common_name {
        flags |= X509CheckFlags::NEVER_CHECK_SUBJECT;
    }
    flags
}

pub fn apply(builder: &mut SslContextBuilder, policy: &ContextPolicy) -> Result<(), TlsError> {
    // CPython forwards the entire native-width masks, including bits absent
    // from the Rust wrappers. OpenSSL owns the meaning of those bits.
    let options = SslOptions::from_bits_retain(policy.options as _);
    let verify = X509VerifyFlags::from_bits_retain(policy.verify_flags as _);
    if options.bits() as u128 != u128::from(policy.options)
        || verify.bits() as u128 != u128::from(policy.verify_flags)
    {
        return Err(invalid(
            "policy mask exceeds the host OpenSSL integer width",
        ));
    }
    // Set the exact mask rather than silently retaining OpenSSL defaults.
    builder.clear_options(builder.options());
    builder.set_options(options);
    let flags = builder.verify_param_mut().flags();
    builder
        .verify_param_mut()
        .clear_flags(flags)
        .map_err(failure)?;
    builder
        .verify_param_mut()
        .set_flags(verify)
        .map_err(failure)?;
    builder
        .verify_param_mut()
        .set_hostflags(host_flags(policy.hostname_common_name));
    let ciphers = policy.ciphers.as_deref().unwrap_or(DEFAULT_CIPHERS);
    if ciphers.len() > BUFFER_LIMIT || ciphers.contains('\0') {
        return Err(invalid("invalid cipher list"));
    }
    builder.set_cipher_list(ciphers).map_err(failure)?;
    Ok(())
}

/// Update a retained native context, preserving its store and session state.
///
/// # Safety
/// The caller must serialize this update with all SSL/SSL_CTX operations that
/// use the context. A shared published context must never be reinterpreted as
/// a uniquely owned SslContextBuilder.
pub unsafe fn apply_shared(
    context: &SslContextRef,
    policy: &ContextPolicy,
) -> Result<(), TlsError> {
    // Validate all input on a private builder before touching the live context.
    let mut validation = SslContextBuilder::new(SslMethod::tls()).map_err(failure)?;
    // SSL_CTX_set_cert_flags(ctx, 0) returns the current flags unchanged.
    // SAFETY: the caller excludes concurrent operations on this context.
    let flags = unsafe { ffi::SSL_CTX_ctrl(context.as_ptr(), 99, 0, std::ptr::null_mut()) };
    seed(
        &mut validation,
        &PolicySeed {
            security_level: context.security_level(),
            suiteb_flags: (flags & 0x30000) as u32,
        },
    )?;
    apply(&mut validation, policy)?;
    let ciphers =
        CString::new(policy.ciphers.as_deref().unwrap_or(DEFAULT_CIPHERS)).map_err(failure)?;
    let pointer = context.as_ptr();
    let param = unsafe { ffi::SSL_CTX_get0_param(pointer) };
    if param.is_null() {
        return Err(failure("context verification parameters unavailable"));
    }
    // SAFETY: OpenSSL's public setters mutate only the retained native context;
    // all associated operations are excluded by the caller's table lock. Flags
    // are copied by SSL_new; cipher selection reads the current context list.
    unsafe {
        if ffi::SSL_CTX_set_cipher_list(pointer, ciphers.as_ptr()) != 1 {
            return Err(failure(openssl::error::ErrorStack::get()));
        }
        ffi::SSL_CTX_clear_options(pointer, ffi::SSL_CTX_get_options(pointer));
        ffi::SSL_CTX_set_options(pointer, policy.options as _);
        let old_flags = ffi::X509_VERIFY_PARAM_get_flags(param);
        let new_flags = policy.verify_flags as libc::c_ulong;
        let clear = old_flags & !new_flags;
        let set = new_flags & !old_flags;
        // CPython sends only deltas. Calling set_flags with an entire mask
        // would incorrectly re-add POLICY_CHECK on repeated policy updates.
        if (clear != 0 && ffi::X509_VERIFY_PARAM_clear_flags(param, clear) != 1)
            || (set != 0 && ffi::X509_VERIFY_PARAM_set_flags(param, set) != 1)
        {
            return Err(failure(openssl::error::ErrorStack::get()));
        }
        ffi::X509_VERIFY_PARAM_set_hostflags(param, host_flags(policy.hostname_common_name).bits());
    }
    Ok(())
}

// Public OpenSSL 1.1.1/3.x accessors absent from the Rust bindings.
unsafe extern "C" {
    fn SSL_get_ciphers(ssl: *const ffi::SSL) -> *mut ffi::stack_st_SSL_CIPHER;
    fn SSL_CIPHER_get_id(cipher: *const ffi::SSL_CIPHER) -> libc::c_ulong;
    fn SSL_CIPHER_is_aead(cipher: *const ffi::SSL_CIPHER) -> libc::c_int;
    fn SSL_CIPHER_get_kx_nid(cipher: *const ffi::SSL_CIPHER) -> libc::c_int;
    fn SSL_CIPHER_get_auth_nid(cipher: *const ffi::SSL_CIPHER) -> libc::c_int;
}

fn nid_name(nid: libc::c_int) -> Result<Option<String>, TlsError> {
    if nid == 0 {
        return Ok(None);
    }
    Nid::from_raw(nid)
        .long_name()
        .map(|name| Some(name.to_owned()))
        .map_err(failure)
}

pub fn inspect(policy: ContextPolicy) -> Result<PolicyDetails, TlsError> {
    let mut builder = SslContextBuilder::new(SslMethod::tls()).map_err(failure)?;
    apply(&mut builder, &policy)?;
    let context = builder.build();
    details(&context)
}

pub fn details(context: &SslContextRef) -> Result<PolicyDetails, TlsError> {
    let ssl = Ssl::new(context).map_err(failure)?;
    // SAFETY: the cipher stack is owned by the live SSL/context and remains
    // borrowed only until this function copies its metadata into owned values.
    let pointer = unsafe { SSL_get_ciphers(ssl.as_ptr()) };
    if pointer.is_null() {
        return Err(failure("OpenSSL returned no cipher stack"));
    }
    let stack = unsafe { StackRef::<SslCipher>::from_ptr(pointer) };
    let mut ciphers = Vec::new();
    for cipher in stack {
        ciphers.push(cipher_details(cipher)?);
    }
    Ok(PolicyDetails {
        ciphers,
        security_level: context.security_level(),
    })
}

pub fn cipher_details(cipher: &SslCipherRef) -> Result<CipherDetails, TlsError> {
    let pointer = cipher.as_ptr();
    let mut description = [0 as libc::c_char; 512];
    // SAFETY: every pointer is a live borrowed SSL_CIPHER; description's
    // buffer length is passed explicitly. OpenSSL produces ASCII text.
    let text = unsafe { ffi::SSL_CIPHER_description(pointer, description.as_mut_ptr(), 511) };
    if text.is_null() {
        return Err(failure("cipher description failed"));
    }
    let description = unsafe { CStr::from_ptr(text) }.to_str().map_err(failure)?;
    let bits = cipher.bits();
    Ok(CipherDetails {
        id: unsafe { SSL_CIPHER_get_id(pointer) } as u64,
        name: cipher.name().into(),
        protocol: cipher.version().into(),
        description: description.strip_suffix('\n').unwrap_or(description).into(),
        strength_bits: bits.secret as u32,
        alg_bits: bits.algorithm as u32,
        aead: unsafe { SSL_CIPHER_is_aead(pointer) } != 0,
        symmetric: cipher
            .cipher_nid()
            .map(|nid| nid_name(nid.as_raw()))
            .transpose()?
            .flatten(),
        digest: nid_name(unsafe { ffi::SSL_CIPHER_get_digest_nid(pointer) })?,
        kea: nid_name(unsafe { SSL_CIPHER_get_kx_nid(pointer) })?,
        auth: nid_name(unsafe { SSL_CIPHER_get_auth_nid(pointer) })?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn policy() -> ContextPolicy {
        ContextPolicy {
            options: 2186412112,
            verify_flags: 32768,
            ciphers: Some("ECDHE-RSA-AES128-GCM-SHA256".into()),
            hostname_common_name: false,
        }
    }
    #[test]
    fn complete_cipher_metadata_and_security_level() {
        let info = inspect(policy()).unwrap();
        assert_eq!(info.security_level, 2);
        let cipher = info
            .ciphers
            .iter()
            .find(|cipher| cipher.name == "ECDHE-RSA-AES128-GCM-SHA256")
            .unwrap();
        assert_eq!(cipher.id, 0x0300c02f);
        assert_eq!(cipher.strength_bits, 128);
        assert_eq!(cipher.alg_bits, 128);
        assert!(cipher.aead);
        assert_eq!(cipher.symmetric.as_deref(), Some("aes-128-gcm"));
        assert_eq!(cipher.digest, None);
        assert_eq!(cipher.kea.as_deref(), Some("kx-ecdhe"));
        assert_eq!(cipher.auth.as_deref(), Some("auth-rsa"));
        assert!(!cipher.description.ends_with('\n'));
        assert!(info
            .ciphers
            .iter()
            .any(|cipher| cipher.protocol == "TLSv1.3"));
    }

    #[test]
    fn shared_policy_updates_keep_context_and_copy_verification_flags() {
        let mut builder = SslContextBuilder::new(SslMethod::tls()).unwrap();
        let original = policy();
        apply(&mut builder, &original).unwrap();
        let context = builder.build();
        let mut before = Ssl::new(&context).unwrap();
        let before_flags = before.param_mut().flags();
        let mut changed = original.clone();
        changed.verify_flags |= 0x4; // OpenSSL X509_V_FLAG_CRL_CHECK, independent of c_ulong width.
        changed.ciphers = Some("ECDHE-RSA-AES256-GCM-SHA384".into());
        // SAFETY: all associated SSL objects remain local and idle during update.
        unsafe { apply_shared(&context, &changed) }.unwrap();
        let mut after = Ssl::new(&context).unwrap();
        assert_eq!(before.param_mut().flags(), before_flags);
        assert!(after
            .param_mut()
            .flags()
            .contains(X509VerifyFlags::CRL_CHECK));
        assert_eq!(before.ssl_context().as_ptr(), after.ssl_context().as_ptr());
        let mut invalid = changed.clone();
        invalid.ciphers = Some("not-a-cipher".into());
        assert!(unsafe { apply_shared(&context, &invalid) }.is_err());
        assert_eq!(context.security_level(), 2);
        changed.ciphers = Some("ECDHE-RSA-AES128-GCM-SHA256:@SECLEVEL=1".into());
        unsafe { apply_shared(&context, &changed) }.unwrap();
        changed.ciphers = Some("ECDHE-RSA-AES128-GCM-SHA256".into());
        unsafe { apply_shared(&context, &changed) }.unwrap();
        assert_eq!(details(&context).unwrap().security_level, 1);
    }
    #[test]
    fn rejects_invalid_masks_and_cipher_lists() {
        // Width is the only mask restriction; Rust's known-bit list is not
        // authoritative for the linked OpenSSL or future native flags.
        let mut p = policy();
        p.options = 1 << 63;
        let too_wide =
            SslOptions::from_bits_retain(p.options as _).bits() as u128 != u128::from(p.options);
        assert_eq!(inspect(p).is_err(), too_wide);
        let mut p = policy();
        p.verify_flags = 1 << 63;
        let too_wide = X509VerifyFlags::from_bits_retain(p.verify_flags as _).bits() as u128
            != u128::from(p.verify_flags);
        assert_eq!(inspect(p).is_err(), too_wide);
        for ciphers in ["not-a-cipher", "name\0tail"] {
            let mut p = policy();
            p.ciphers = Some(ciphers.into());
            assert!(inspect(p).is_err());
        }
    }

    #[test]
    fn raw_mask_updates_copy_ssl_flags_and_clear_unlisted_bits() {
        if std::mem::size_of::<libc::c_ulong>() < 8 {
            return; // Native-width overflow is covered separately above.
        }
        let original = policy();
        let mut builder = SslContextBuilder::new(SslMethod::tls()).unwrap();
        apply(&mut builder, &original).unwrap();
        let context = builder.build();
        let mut before = Ssl::new(&context).unwrap();
        let mut changed = original.clone();
        changed.options = (1 << 63) | 4 | 8;
        changed.verify_flags = (1 << 63) | original.verify_flags;
        // SAFETY: local resources only, no concurrent native operations.
        unsafe { apply_shared(&context, &changed) }.unwrap();
        let mut after = Ssl::new(&context).unwrap();
        let options = || u128::from(unsafe { ffi::SSL_CTX_get_options(context.as_ptr()) });
        assert_eq!(options(), u128::from(changed.options));
        assert_eq!(
            u128::from(before.param_mut().flags().bits()),
            u128::from(original.verify_flags)
        );
        assert_eq!(
            u128::from(after.param_mut().flags().bits()),
            u128::from(changed.verify_flags)
        );
        assert_eq!(before.ssl_context().as_ptr(), after.ssl_context().as_ptr());
        let mut invalid = original.clone();
        invalid.ciphers = Some("not-a-cipher".into());
        assert!(unsafe { apply_shared(&context, &invalid) }.is_err());
        assert_eq!(options(), u128::from(changed.options));
        unsafe { apply_shared(&context, &original) }.unwrap();
        assert_eq!(options(), u128::from(original.options));
        let mut cleared = Ssl::new(&context).unwrap();
        assert_eq!(
            u128::from(cleared.param_mut().flags().bits()),
            u128::from(original.verify_flags)
        );
        assert_eq!(
            u128::from(after.param_mut().flags().bits()),
            u128::from(changed.verify_flags)
        );
        assert_eq!(context.security_level(), 2);
    }

    #[test]
    fn inherited_suiteb_flags_validate_raw_expressions() {
        for (mode, flags, length) in [
            ("SUITEB128", 0x30000, 2),
            ("SUITEB128ONLY", 0x10000, 1),
            ("SUITEB128C2", 0x30000, 2),
            ("SUITEB192", 0x20000, 1),
        ] {
            let mut builder = SslContextBuilder::new(SslMethod::tls()).unwrap();
            builder.set_security_level(1);
            let mut p = policy();
            p.ciphers = Some(format!("{mode}:@SECLEVEL=2"));
            apply(&mut builder, &p).unwrap();
            let context = builder.build();
            p.ciphers = Some("not-a-cipher:@SECLEVEL=2".into());
            // SAFETY: this context remains local with no concurrent users.
            unsafe { apply_shared(&context, &p) }.unwrap();
            let actual = details(&context).unwrap();
            let seeded = inspect_seeded(SeededPolicy {
                policy: p,
                seed: PolicySeed {
                    security_level: 1,
                    suiteb_flags: flags,
                },
            })
            .unwrap();
            assert_eq!(actual.security_level, 1);
            assert_eq!(seeded.security_level, 1);
            let names = |p: PolicyDetails| {
                p.ciphers
                    .into_iter()
                    .filter(|c| c.protocol != "TLSv1.3")
                    .map(|c| c.name)
                    .collect::<Vec<_>>()
            };
            let actual = names(actual);
            assert_eq!(actual.len(), length);
            assert_eq!(actual, names(seeded));
        }
        for seed in [
            PolicySeed {
                security_level: 6,
                suiteb_flags: 0,
            },
            PolicySeed {
                security_level: 2,
                suiteb_flags: 1,
            },
        ] {
            assert!(inspect_seeded(SeededPolicy {
                policy: policy(),
                seed
            })
            .is_err());
        }
    }
}
