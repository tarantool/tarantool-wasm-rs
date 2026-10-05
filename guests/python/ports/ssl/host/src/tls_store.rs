//! Guest-owned trust bundles. No paths, sockets or password prompts.
use crate::bindings::ssl_bio::{StoreCertificate, StoreDetails, StoreQuery, TlsError};
use crate::tls_bio::{failure, invalid, BUFFER_LIMIT};
use foreign_types::{ForeignType, ForeignTypeRef};
use openssl::{
    error::ErrorStack,
    stack::StackRef,
    x509::{
        store::{X509StoreBuilder, X509StoreBuilderRef, X509StoreRef},
        X509Crl, X509Object, X509,
    },
};
use openssl_sys as ffi;

// Stable OpenSSL 1.1.1/3.x public APIs absent from openssl-sys.
unsafe extern "C" {
    fn X509_STORE_add_crl(store: *mut ffi::X509_STORE, crl: *mut ffi::X509_CRL) -> libc::c_int;
    fn X509_OBJECT_get_type(object: *const ffi::X509_OBJECT) -> libc::c_int;
    fn X509_check_ca(cert: *mut ffi::X509) -> libc::c_int;
    fn X509_STORE_lock(store: *mut ffi::X509_STORE) -> libc::c_int;
    fn X509_STORE_unlock(store: *mut ffi::X509_STORE) -> libc::c_int;
}

pub(super) struct StoreLock<'a>(&'a X509StoreRef);

impl<'a> StoreLock<'a> {
    pub(super) fn acquire(store: &'a X509StoreRef) -> Result<Self, TlsError> {
        // SAFETY: the borrow keeps this store alive until the lock is released.
        if unsafe { X509_STORE_lock(store.as_ptr()) } != 1 {
            return Err(failure("cannot lock TLS trust store"));
        }
        Ok(Self(store))
    }
}

impl Drop for StoreLock<'_> {
    fn drop(&mut self) {
        // SAFETY: this guard owns a successful matching store lock.
        unsafe { X509_STORE_unlock(self.0.as_ptr()) };
    }
}

fn bounded(size: usize) -> Result<(), TlsError> {
    if size > BUFFER_LIMIT {
        Err(invalid("TLS trust store limit exceeded"))
    } else {
        Ok(())
    }
}

pub(super) enum Entry {
    Certificate(X509),
    Crl(X509Crl),
}

/// Visit complete recognized PEM blocks, ignoring text and unrelated PEM objects.
/// Retaining the validated certificate block preserves trusted-certificate AUX.
pub(super) fn entries(data: &[u8]) -> Result<Vec<(Entry, Vec<u8>)>, TlsError> {
    selected_entries(data, None)
}

pub(super) fn directory_entries(data: &[u8], crl: bool) -> Result<Vec<(Entry, Vec<u8>)>, TlsError> {
    selected_entries(data, Some(crl))
}

fn selected_entries(data: &[u8], crl: Option<bool>) -> Result<Vec<(Entry, Vec<u8>)>, TlsError> {
    bounded(data.len())?;
    let mut lines = data.split_inclusive(|byte| *byte == b'\n');
    let mut result = Vec::new();
    while let Some(line) = lines.next() {
        let label = line.trim_ascii_end();
        let Some(label) = label
            .strip_prefix(b"-----BEGIN ")
            .and_then(|s| s.strip_suffix(b"-----"))
        else {
            continue;
        };
        let recognized = matches!(
            label,
            b"CERTIFICATE" | b"X509 CERTIFICATE" | b"TRUSTED CERTIFICATE" | b"X509 CRL"
        ) && crl.is_none_or(|crl| crl == (label == b"X509 CRL"));
        let mut block = line.to_vec();
        let mut end = b"-----END ".to_vec();
        end.extend_from_slice(label);
        end.extend_from_slice(b"-----");
        let mut complete = false;
        for line in lines.by_ref() {
            block.extend_from_slice(line);
            let stripped = line.trim_ascii_end();
            if stripped == end {
                complete = true;
                break;
            }
        }
        if !recognized {
            continue;
        }
        if !complete {
            return Err(failure("incomplete trust store PEM block"));
        }
        let entry = if label == b"X509 CRL" {
            Entry::Crl(crate::tls_pem::crl(&block)?)
        } else {
            Entry::Certificate(crate::tls_pem::trusted_certificate(&block)?)
        };
        if !block.ends_with(b"\n") {
            block.push(b'\n');
        }
        result.push((entry, block));
    }
    if result.is_empty() {
        return Err(failure("trust bundle contains no certificates or CRLs"));
    }
    Ok(result)
}

pub fn prepare(data: Vec<u8>) -> Result<Vec<u8>, TlsError> {
    let mut output = Vec::new();
    for (_, block) in entries(&data)? {
        bounded(output.len() + block.len())?;
        output.extend(block);
    }
    Ok(output)
}

pub fn add(store: &mut X509StoreBuilderRef, data: &[u8]) -> Result<(), TlsError> {
    for (entry, _) in entries(data)? {
        match entry {
            Entry::Certificate(cert) => store.add_cert(cert).map_err(failure)?,
            Entry::Crl(crl) => {
                // SAFETY: store and CRL are live owned values. Successful add
                // increments the CRL refcount; the local reference is dropped.
                if unsafe { X509_STORE_add_crl(store.as_ptr(), crl.as_ptr()) } != 1 {
                    return Err(failure(ErrorStack::get()));
                }
            }
        }
    }
    Ok(())
}

/// Add to the store shared by an existing SSLContext and its connections.
/// Parse the entire bounded bundle first. The native add APIs acquire the
/// store lock internally and retain their own certificate/CRL references.
#[derive(Default)]
pub(super) struct StoreBudget {
    loaded: std::collections::HashMap<[u8; 32], usize>,
    bytes: usize,
}

impl StoreBudget {
    /// Eager bundles plus lazy directory entries may retain at most 2 MiB
    /// of guest PEM and 8192 distinct objects. Defaults are an explicit host
    /// capability and are not charged to this guest-input budget.
    pub fn add(
        &mut self,
        store: &X509StoreRef,
        entries: Vec<(Entry, Vec<u8>)>,
    ) -> Result<(), TlsError> {
        let mut pending = std::collections::HashMap::new();
        let mut prepared = Vec::new();
        for (entry, block) in entries {
            let (tag, mut der) = match &entry {
                Entry::Certificate(cert) => (1, cert.to_der().map_err(failure)?),
                Entry::Crl(crl) => (2, crl.to_der().map_err(failure)?),
            };
            der.push(tag);
            let digest = openssl::sha::sha256(&der);
            if !self.loaded.contains_key(&digest) && !pending.contains_key(&digest) {
                pending.insert(digest, block.len());
                prepared.push((digest, block.len(), entry));
            }
        }
        if self.loaded.len() + pending.len() > 8192
            || self.bytes.saturating_add(pending.values().sum::<usize>()) > BUFFER_LIMIT * 2
        {
            return Err(invalid("TLS retained trust store limit exceeded"));
        }
        for (digest, size, entry) in prepared {
            let ok = match entry {
                Entry::Certificate(cert) => unsafe {
                    ffi::X509_STORE_add_cert(store.as_ptr(), cert.as_ptr())
                },
                Entry::Crl(crl) => unsafe { X509_STORE_add_crl(store.as_ptr(), crl.as_ptr()) },
            };
            if ok != 1 {
                return Err(failure(ErrorStack::get()));
            }
            self.loaded.insert(digest, size);
            self.bytes += size;
        }
        Ok(())
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;

    #[test]
    fn retained_guest_trust_is_bounded_and_failed_addition_keeps_store() {
        let store = X509StoreBuilder::new().unwrap().build();
        let mut budget = StoreBudget::default();
        for pem in [
            include_bytes!("../../../../tests/fixtures/ssl/server.crt").as_slice(),
            include_bytes!("../../../../tests/fixtures/ssl/ca.crt").as_slice(),
        ] {
            let mut values = entries(pem).unwrap();
            // Account for a large but valid PEM block (e.g. whitespace).
            values[0].1.resize(BUFFER_LIMIT, b' ');
            budget.add(&store, values).unwrap();
            budget.add(&store, entries(pem).unwrap()).unwrap();
        }
        assert_eq!(details(&store, StoreQuery::Stats).unwrap().x509, 2);
        assert!(budget
            .add(
                &store,
                entries(include_bytes!("../../../../tests/fixtures/ssl/leaf.crt")).unwrap()
            )
            .is_err());
        assert_eq!(details(&store, StoreQuery::Stats).unwrap().x509, 2);
    }
}

// Public OpenSSL 1.1.1/3.x APIs; no guest-controlled host paths.
unsafe extern "C" {
    fn SSL_CTX_set_default_verify_file(ctx: *mut ffi::SSL_CTX) -> libc::c_int;
    fn SSL_CTX_set_default_verify_dir(ctx: *mut ffi::SSL_CTX) -> libc::c_int;
}

/// Caller serializes native context and connection operations with the same
/// resource-table guard. Loading is additive, as in set_default_verify_paths.
pub(super) fn load_defaults(
    context: &openssl::ssl::SslContextRef,
    sources: crate::bindings::ssl_bio::DefaultTrustSources,
) -> Result<(), TlsError> {
    if sources.file && sources.directory {
        // Preserve OpenSSL 3's default store URI as well as file/directory.
        if unsafe { ffi::SSL_CTX_set_default_verify_paths(context.as_ptr()) } != 1 {
            return Err(failure(ErrorStack::get()));
        }
        return Ok(());
    }
    if sources.file && unsafe { SSL_CTX_set_default_verify_file(context.as_ptr()) } != 1 {
        return Err(failure(ErrorStack::get()));
    }
    if sources.directory && unsafe { SSL_CTX_set_default_verify_dir(context.as_ptr()) } != 1 {
        return Err(failure(ErrorStack::get()));
    }
    Ok(())
}

pub fn inspect_with_defaults(
    data: Option<Vec<u8>>,
    sources: crate::bindings::ssl_bio::DefaultTrustSources,
    query: StoreQuery,
) -> Result<StoreDetails, TlsError> {
    let mut context =
        openssl::ssl::SslContextBuilder::new(openssl::ssl::SslMethod::tls()).map_err(failure)?;
    if let Some(data) = data {
        add(context.cert_store_mut(), &data)?;
    }
    let context = context.build();
    load_defaults(&context, sources)?;
    details(context.cert_store(), query)
}

pub fn inspect(
    data: Option<Vec<u8>>,
    default_certs: bool,
    query: StoreQuery,
) -> Result<StoreDetails, TlsError> {
    let mut store = X509StoreBuilder::new().map_err(failure)?;
    if default_certs {
        store.set_default_paths().map_err(failure)?;
    }
    if let Some(data) = data {
        add(&mut store, &data)?;
    }
    let store = store.build();
    details(&store, query)
}

pub fn details(store: &X509StoreRef, query: StoreQuery) -> Result<StoreDetails, TlsError> {
    let _lock = StoreLock::acquire(store)?;
    let mut result = StoreDetails {
        x509: 0,
        x509_ca: 0,
        crl: 0,
        certificates: Vec::new(),
    };
    let mut output_size = 0usize;
    // The native store lock excludes concurrent lazy lookups/cache mutation.
    let pointer = unsafe { ffi::X509_STORE_get0_objects(store.as_ptr()) };
    if pointer.is_null() {
        return Err(failure("trust store object stack is unavailable"));
    }
    // SAFETY: the stack stays borrowed under the native store lock; no pointer
    // or borrowed certificate escapes this synchronous enumeration.
    let objects = unsafe { StackRef::<X509Object>::from_ptr(pointer) };
    for object in objects {
        if let Some(cert) = object.x509() {
            result.x509 += 1;
            // SAFETY: cert remains owned by the live private store.
            if unsafe { X509_check_ca(cert.as_ptr()) } == 0 {
                continue;
            }
            result.x509_ca += 1;
            if matches!(query, StoreQuery::Stats) {
                continue;
            }
            let der = cert.to_der().map_err(failure)?;
            output_size = output_size.saturating_add(der.len());
            bounded(output_size)?;
            result.certificates.push(match query {
                StoreQuery::Der => StoreCertificate::Der(der),
                StoreQuery::Decoded => {
                    StoreCertificate::Decoded(crate::tls_certificate::decode(cert)?)
                }
                StoreQuery::Stats => unreachable!(),
            });
        } else {
            // SAFETY: object is borrowed from the live private store. Public
            // X509_LOOKUP_TYPE uses 2 for a CRL in OpenSSL 1.1.1 and 3.x.
            if unsafe { X509_OBJECT_get_type(object.as_ptr()) } == 2 {
                result.crl += 1;
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    const CA: &[u8] = include_bytes!("../../../../tests/fixtures/ssl/server.crt");
    #[test]
    fn store_deduplicates_and_never_decrypts_unrelated_keys() {
        let bundle = [
            CA,
            CA,
            include_bytes!("../../../../tests/fixtures/ssl/server-encrypted.key"),
        ]
        .concat();
        let normalized = prepare(bundle).unwrap();
        let result = inspect(Some(normalized), false, StoreQuery::Der).unwrap();
        assert_eq!((result.x509, result.x509_ca, result.crl), (1, 1, 0));
        assert_eq!(result.certificates.len(), 1);
        assert!(prepare(
            b"-----BEGIN CERTIFICATE-----\ninvalid\n-----END CERTIFICATE-----".to_vec()
        )
        .is_err());
        assert!(prepare(vec![0; BUFFER_LIMIT + 1]).is_err());
        let padded =
            prepare(include_bytes!("../../../../tests/fixtures/ssl/padded-store.pem").to_vec())
                .unwrap();
        assert_eq!(
            inspect(Some(padded), false, StoreQuery::Stats).unwrap().crl,
            1
        );
    }

    #[test]
    fn mixed_bundle_retains_crls_aux_and_ca_classification() {
        let bundle = [
            include_bytes!("../../../../tests/fixtures/ssl/rejected-ca.pem").as_slice(),
            include_bytes!("../../../../tests/fixtures/ssl/leaf.crt").as_slice(),
            include_bytes!("../../../../tests/fixtures/ssl/empty.crl").as_slice(),
            include_bytes!("../../../../tests/fixtures/ssl/empty.crl").as_slice(),
        ]
        .concat();
        let normalized = prepare(bundle).unwrap();
        assert!(normalized
            .windows(b"TRUSTED CERTIFICATE".len())
            .any(|w| w == b"TRUSTED CERTIFICATE"));
        let result = inspect(Some(normalized), false, StoreQuery::Decoded).unwrap();
        assert_eq!((result.x509, result.x509_ca, result.crl), (2, 1, 1));
        assert_eq!(result.certificates.len(), 1);
        let only_crl =
            prepare(include_bytes!("../../../../tests/fixtures/ssl/empty.crl").to_vec()).unwrap();
        assert_eq!(
            inspect(Some(only_crl), false, StoreQuery::Stats)
                .unwrap()
                .crl,
            1
        );
        assert!(
            prepare(b"-----BEGIN X509 CRL-----\ninvalid\n-----END X509 CRL-----\n".to_vec())
                .is_err()
        );
    }
}
