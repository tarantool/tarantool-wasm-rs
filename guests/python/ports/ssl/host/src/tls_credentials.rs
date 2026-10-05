//! Parse and normalize guest-owned TLS credentials without host files or stdin.
use crate::bindings::ssl_bio::{CertificateInput, Credentials, PreparedCredentials, TlsError};
use crate::tls_bio::{failure, invalid, BUFFER_LIMIT};
use foreign_types::{ForeignType, ForeignTypeRef};
use openssl::{
    error::ErrorStack,
    pkey::{PKey, Private},
    ssl::{SslContextBuilder, SslContextRef, SslMethod},
    stack::Stack,
    x509::X509,
};
use openssl_sys as ffi;

extern "C" {
    fn X509_check_private_key(cert: *const ffi::X509, key: *const ffi::EVP_PKEY) -> libc::c_int;
    fn SSL_CTX_use_cert_and_key(
        ctx: *mut ffi::SSL_CTX,
        leaf: *mut ffi::X509,
        key: *mut ffi::EVP_PKEY,
        chain: *mut ffi::stack_st_X509,
        replace: libc::c_int,
    ) -> libc::c_int;
}

pub(super) struct Parsed {
    leaf: X509,
    key: PKey<Private>,
    chain: Stack<X509>,
    pub key_type: u32,
    pub size: usize,
}

pub(super) fn parse(value: &Credentials) -> Result<Parsed, TlsError> {
    let size = value.cert_pem.len().saturating_add(value.key_pem.len());
    bounded(size)?;
    let mut certificates = crate::tls_pem::certificates(&value.cert_pem)?.into_iter();
    let leaf = certificates
        .next()
        .ok_or_else(|| failure("PEM lib: empty certificate chain"))?;
    let key = PKey::private_key_from_pem_callback(&value.key_pem, |_| Err(ErrorStack::get()))
        .map_err(failure)?;
    let public = leaf.public_key().map_err(failure)?;
    if !public.public_eq(&key) {
        return Err(failure("key values mismatch"));
    }
    let key_type =
        u32::try_from(public.id().as_raw()).map_err(|_| invalid("invalid certificate key type"))?;
    let mut chain = Stack::new().map_err(failure)?;
    for certificate in certificates {
        chain.push(certificate).map_err(failure)?;
    }
    Ok(Parsed {
        leaf,
        key,
        chain,
        key_type,
        size,
    })
}

impl Parsed {
    /// # Safety
    /// Caller must exclude every associated native SSL/SSL_CTX operation.
    pub unsafe fn install(&self, context: &SslContextRef) -> Result<(), TlsError> {
        // SAFETY: public API validates all security/key requirements and
        // allocates a retained chain before replacing any credential fields.
        // It increments references; the owned inputs remain live for the call.
        if unsafe {
            SSL_CTX_use_cert_and_key(
                context.as_ptr(),
                self.leaf.as_ptr(),
                self.key.as_ptr(),
                if self.chain.is_empty() {
                    std::ptr::null_mut()
                } else {
                    self.chain.as_ptr()
                },
                1,
            )
        } == 1
        {
            Ok(())
        } else {
            Err(failure(ErrorStack::get()))
        }
    }
}

pub fn prepare_policy_credentials(
    cert: Vec<u8>,
    key: Vec<u8>,
    password: Option<Vec<u8>>,
    policy: crate::bindings::ssl_bio::SeededPolicy,
) -> Result<crate::bindings::ssl_bio::PreparedPolicyCredentials, TlsError> {
    use crate::bindings::ssl_bio::{PreparedPolicyCredentials, ValidatedCredentials};
    bounded(
        cert.len()
            .saturating_add(key.len())
            .saturating_add(password.as_ref().map_or(0, Vec::len)),
    )?;
    let mut validation = SslContextBuilder::new(SslMethod::tls()).map_err(failure)?;
    crate::tls_policy::seed(&mut validation, &policy.seed)?;
    crate::tls_policy::apply(&mut validation, &policy.policy)?;
    // Like CPython's file loader, reject weak public certificates before
    // requesting/decrypting an encrypted private key in the guest.
    let mut certificates = crate::tls_pem::certificates(&cert)?.into_iter();
    let leaf = certificates
        .next()
        .ok_or_else(|| failure("PEM lib: empty certificate chain"))?;
    validation.set_certificate(&leaf).map_err(failure)?;
    for certificate in certificates {
        // SAFETY: public SSL_CTX_add1_chain_cert macro, on an unpublished
        // builder. Unlike extra-chain addition it checks security policy.
        // add1 retains a reference only on success; this owner always drops.
        if unsafe {
            ffi::SSL_CTX_ctrl(
                validation.as_ptr(),
                ffi::SSL_CTRL_CHAIN_CERT,
                1,
                certificate.as_ptr().cast(),
            )
        } != 1
        {
            return Err(failure(ErrorStack::get()));
        }
    }
    let PreparedCredentials::Ready(credentials) = prepare_credentials(cert, key, password)? else {
        return Ok(PreparedPolicyCredentials::PasswordRequired);
    };
    let parsed = parse(&credentials)?;
    let validation = validation.build();
    // SAFETY: the private context has not escaped and has no SSL references.
    unsafe { parsed.install(&validation) }?;
    Ok(PreparedPolicyCredentials::Ready(ValidatedCredentials {
        credentials,
        key_type: parsed.key_type,
    }))
}

fn bounded(size: usize) -> Result<(), TlsError> {
    if size > BUFFER_LIMIT {
        Err(invalid("TLS credential limit exceeded"))
    } else {
        Ok(())
    }
}

fn pem(certs: &[X509]) -> Result<Vec<u8>, TlsError> {
    if certs.is_empty() {
        return Err(failure("certificate input contains no certificates"));
    }
    let mut result = Vec::new();
    for cert in certs {
        let bytes = cert.to_pem().map_err(failure)?;
        bounded(result.len() + bytes.len())?;
        result.extend(bytes);
    }
    Ok(result)
}

pub fn prepare_ca(data: CertificateInput) -> Result<Vec<u8>, TlsError> {
    let certs = match data {
        CertificateInput::Pem(data) => {
            bounded(data.len())?;
            if data.is_empty() {
                return Err(invalid("Empty certificate data"));
            }
            match crate::tls_pem::certificates(&data) {
                Ok(certs) => certs,
                Err(error) => {
                    // Distinguish a failed first certificate from a malformed
                    // tail after valid certificates without losing its error.
                    if matches!(error.kind, crate::bindings::ssl_bio::ErrorKind::Failure)
                        && crate::tls_pem::first_certificate(&data).is_err()
                    {
                        return Err(failure("certificate input contains no certificates"));
                    }
                    return Err(error);
                }
            }
        }
        CertificateInput::Der(data) => {
            bounded(data.len())?;
            if data.is_empty() {
                return Err(invalid("Empty certificate data"));
            }
            openssl::init();
            let mut remaining = data.as_slice();
            let mut certs = Vec::new();
            while !remaining.is_empty() {
                let start = remaining.as_ptr();
                let mut cursor = start;
                // SAFETY: d2i_X509 reads at most the supplied bounded slice
                // length and advances cursor within it. A non-null result is
                // a newly allocated X509, transferred to an owned Rust value.
                let pointer = unsafe {
                    ffi::d2i_X509(
                        std::ptr::null_mut(),
                        &mut cursor,
                        remaining.len() as libc::c_long,
                    )
                };
                if pointer.is_null() {
                    let errors = ErrorStack::get();
                    return Err(if certs.is_empty() {
                        failure("certificate input contains no certificates")
                    } else {
                        failure(errors)
                    });
                }
                let cert = unsafe { X509::from_ptr(pointer) };
                let consumed = (cursor as usize)
                    .checked_sub(start as usize)
                    .filter(|count| *count > 0 && *count <= remaining.len())
                    .ok_or_else(|| failure("invalid DER certificate length"))?;
                certs.push(cert);
                remaining = &remaining[consumed..];
            }
            certs
        }
    };
    pem(&certs)
}

pub fn prepare_credentials(
    cert: Vec<u8>,
    key: Vec<u8>,
    password: Option<Vec<u8>>,
) -> Result<PreparedCredentials, TlsError> {
    bounded(
        cert.len()
            .saturating_add(key.len())
            .saturating_add(password.as_ref().map_or(0, Vec::len)),
    )?;
    let certs = crate::tls_pem::certificates(&cert)?;
    let leaf = certs
        .first()
        .ok_or_else(|| failure("PEM lib: empty certificate chain"))?;
    let mut password_required = false;
    let mut password_too_long = false;
    let parsed = PKey::private_key_from_pem_callback(&key, |buffer| {
        let Some(password) = password.as_deref() else {
            password_required = true;
            return Err(ErrorStack::get());
        };
        if password.len() > buffer.len() {
            password_too_long = true;
            return Err(ErrorStack::get());
        }
        buffer[..password.len()].copy_from_slice(password);
        Ok(password.len())
    });
    if password_required && parsed.is_err() {
        return Ok(PreparedCredentials::PasswordRequired);
    }
    if password_too_long {
        return Err(invalid("password exceeds OpenSSL password buffer"));
    }
    // CPython's file-style loader reports PEM lib for decoder failures.
    // Retain the serialized native stack for symbolic library/reason lookup.
    let key = parsed.map_err(|error| failure(format!("{error}, PEM lib")))?;
    // SAFETY: both borrowed objects remain owned for this synchronous check.
    // Preserve the native error code instead of inventing a mismatch message.
    if unsafe { X509_check_private_key(leaf.as_ptr(), key.as_ptr()) } != 1 {
        return Err(failure(ErrorStack::get()));
    }
    let cert_pem = pem(&certs)?;
    let key_pem = key.private_key_to_pem_pkcs8().map_err(failure)?;
    bounded(cert_pem.len() + key_pem.len())?;
    Ok(PreparedCredentials::Ready(Credentials {
        cert_pem,
        key_pem,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    const CERT: &[u8] = include_bytes!("../../../../tests/fixtures/ssl/server.crt");
    const KEY: &[u8] = include_bytes!("../../../../tests/fixtures/ssl/server.key");

    #[test]
    fn ca_der_sequences_and_invalid_tail() {
        let cert = X509::from_pem(CERT).unwrap();
        let der = cert.to_der().unwrap();
        let two = [der.as_slice(), der.as_slice()].concat();
        let normalized = prepare_ca(CertificateInput::Der(two)).unwrap();
        assert_eq!(X509::stack_from_pem(&normalized).unwrap().len(), 2);
        let invalid = [der.as_slice(), b"invalid"].concat();
        assert_ne!(
            prepare_ca(CertificateInput::Der(invalid))
                .unwrap_err()
                .message,
            "certificate input contains no certificates"
        );
        for input in [
            CertificateInput::Der(b"broken".to_vec()),
            CertificateInput::Pem(b"broken".to_vec()),
        ] {
            assert_eq!(
                prepare_ca(input).unwrap_err().message,
                "certificate input contains no certificates"
            );
        }
        let broken_pem = b"-----BEGIN CERTIFICATE-----\nnot base64!\n-----END CERTIFICATE-----\n";
        assert_eq!(
            prepare_ca(CertificateInput::Pem(broken_pem.to_vec()))
                .unwrap_err()
                .message,
            "certificate input contains no certificates"
        );
        assert_ne!(
            prepare_ca(CertificateInput::Pem([CERT, broken_pem].concat()))
                .unwrap_err()
                .message,
            "certificate input contains no certificates"
        );
        assert!(prepare_ca(CertificateInput::Pem(vec![])).is_err());
        assert!(prepare_ca(CertificateInput::Pem(vec![0; BUFFER_LIMIT + 1])).is_err());
    }

    #[test]
    fn decrypt_password_bytes_and_never_prompt() {
        let key = PKey::private_key_from_pem(KEY).unwrap();
        let password = b"unicode-\xD0\xBF\0password";
        let encrypted = key
            .private_key_to_pem_pkcs8_passphrase(openssl::symm::Cipher::aes_256_cbc(), password)
            .unwrap();
        assert!(matches!(
            prepare_credentials(CERT.to_vec(), encrypted.clone(), None).unwrap(),
            PreparedCredentials::PasswordRequired
        ));
        assert!(
            prepare_credentials(CERT.to_vec(), encrypted.clone(), Some(b"wrong".to_vec())).is_err()
        );
        let PreparedCredentials::Ready(credentials) =
            prepare_credentials(CERT.to_vec(), encrypted, Some(password.to_vec())).unwrap()
        else {
            panic!("password supplied but not accepted");
        };
        assert!(key.public_eq(&PKey::private_key_from_pem(&credentials.key_pem).unwrap()));
        let other = PKey::from_rsa(openssl::rsa::Rsa::generate(2048).unwrap()).unwrap();
        assert!(prepare_credentials(
            CERT.to_vec(),
            other.private_key_to_pem_pkcs8().unwrap(),
            None
        )
        .is_err());
    }
}
