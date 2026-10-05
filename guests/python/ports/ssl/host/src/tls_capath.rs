//! Lazy OpenSSL hashed-directory lookup over bounded guest bytes, never paths.
use crate::bindings::ssl_bio::{TlsError, TrustDirectory};
use crate::{
    tls_bio::{failure, invalid, BUFFER_LIMIT},
    tls_store::{StoreBudget, StoreLock},
};
use foreign_types::ForeignTypeRef;
use openssl::{
    error::ErrorStack,
    x509::store::{X509StoreBuilderRef, X509StoreRef},
};
use openssl_sys as ffi;
use std::{
    collections::HashMap,
    ptr,
    sync::{Arc, Mutex, OnceLock, RwLock},
};

// These are opaque pointers in the public OpenSSL 1.1.1/3.x lookup API.
type Lookup = libc::c_void;
type Method = libc::c_void;
unsafe extern "C" {
    fn X509_LOOKUP_meth_new(name: *const libc::c_char) -> *mut Method;
    fn X509_LOOKUP_meth_free(method: *mut Method);
    fn X509_LOOKUP_meth_set_free(
        method: *mut Method,
        free: unsafe extern "C" fn(*mut Lookup),
    ) -> libc::c_int;
    fn X509_LOOKUP_meth_set_get_by_subject(
        method: *mut Method,
        get: unsafe extern "C" fn(
            *mut Lookup,
            libc::c_int,
            *const ffi::X509_NAME,
            *mut ffi::X509_OBJECT,
        ) -> libc::c_int,
    ) -> libc::c_int;
    fn X509_STORE_add_lookup(store: *mut ffi::X509_STORE, method: *mut Method) -> *mut Lookup;
    fn X509_LOOKUP_set_method_data(lookup: *mut Lookup, data: *mut libc::c_void) -> libc::c_int;
    fn X509_LOOKUP_get_method_data(lookup: *const Lookup) -> *mut libc::c_void;
    fn X509_LOOKUP_get_store(lookup: *const Lookup) -> *mut ffi::X509_STORE;
    fn X509_OBJECT_retrieve_by_subject(
        objects: *mut ffi::stack_st_X509_OBJECT,
        kind: libc::c_int,
        name: *const ffi::X509_NAME,
    ) -> *mut ffi::X509_OBJECT;
    fn X509_OBJECT_get0_X509_CRL(object: *const ffi::X509_OBJECT) -> *mut ffi::X509_CRL;
    fn X509_OBJECT_set1_X509(object: *mut ffi::X509_OBJECT, cert: *mut ffi::X509) -> libc::c_int;
    fn X509_OBJECT_set1_X509_CRL(
        object: *mut ffi::X509_OBJECT,
        crl: *mut ffi::X509_CRL,
    ) -> libc::c_int;
}

struct Snapshot {
    directories: Vec<HashMap<String, Vec<u8>>>,
}

/// The lookup and context each retain a reference. Connections keep the lookup
/// alive through OpenSSL's store reference even after the context is dropped.
#[derive(Clone)]
pub struct DirectoryLookup {
    snapshot: Arc<RwLock<Snapshot>>,
    budget: Arc<Mutex<StoreBudget>>,
}

impl DirectoryLookup {
    pub fn add_pem(&self, store: &X509StoreRef, pem: &[u8]) -> Result<(), TlsError> {
        self.budget
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .add(store, crate::tls_store::entries(pem)?)
    }
    pub fn update(
        &self,
        store: &X509StoreRef,
        pem: Option<&[u8]>,
        directories: Vec<TrustDirectory>,
    ) -> Result<(), TlsError> {
        let snapshot = validated_snapshot(directories)?;
        if let Some(pem) = pem {
            self.add_pem(store, pem)?;
        }
        *self.snapshot.write().unwrap_or_else(|p| p.into_inner()) = snapshot;
        Ok(())
    }
}

fn valid_name(name: &str) -> bool {
    let Some((hash, suffix)) = name.split_once('.') else {
        return false;
    };
    if hash.len() != 8
        || !hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return false;
    }
    let suffix = suffix.strip_prefix('r').unwrap_or(suffix);
    suffix.parse::<u32>().is_ok_and(|n| suffix == n.to_string())
}

fn method() -> Result<*mut Method, TlsError> {
    static METHOD: OnceLock<Result<usize, TlsError>> = OnceLock::new();
    let result = METHOD.get_or_init(|| {
        openssl::init();
        // SAFETY: the registered method is immutable and lives for the process,
        // like the library's built-in lookup methods. Per-store data is owned
        // separately and released by free_snapshot at the final store drop.
        let method = unsafe { X509_LOOKUP_meth_new(c"guest hashed CA directory".as_ptr()) };
        if method.is_null() {
            return Err(failure(ErrorStack::get()));
        }
        if unsafe { X509_LOOKUP_meth_set_free(method, free_snapshot) } != 1
            || unsafe { X509_LOOKUP_meth_set_get_by_subject(method, get_by_subject) } != 1
        {
            unsafe { X509_LOOKUP_meth_free(method) };
            return Err(failure(ErrorStack::get()));
        }
        Ok(method as usize)
    });
    result.clone().map(|p| p as *mut Method)
}

fn validated_snapshot(directories: Vec<TrustDirectory>) -> Result<Snapshot, TlsError> {
    if directories.len() > 16 {
        return Err(invalid("TLS CA directory count exceeds 16"));
    }
    let mut bytes = 0usize;
    let mut files = 0usize;
    let mut snapshots = Vec::new();
    for directory in directories {
        let mut snapshot = HashMap::new();
        for file in directory.files {
            files += 1;
            bytes = bytes
                .saturating_add(file.name.len())
                .saturating_add(file.data.len());
            if files > 4096 || bytes > BUFFER_LIMIT {
                return Err(invalid("TLS CA directory snapshot limit exceeded"));
            }
            if !valid_name(&file.name) {
                return Err(invalid("invalid hashed CA basename"));
            }
            if snapshot.insert(file.name, file.data).is_some() {
                return Err(invalid("duplicate hashed CA basename"));
            }
        }
        snapshots.push(snapshot);
    }
    Ok(Snapshot {
        directories: snapshots,
    })
}

pub fn install(
    store: &mut X509StoreBuilderRef,
    directories: Vec<TrustDirectory>,
) -> Result<DirectoryLookup, TlsError> {
    let snapshot = validated_snapshot(directories)?;
    let method = method()?;
    // SAFETY: installation runs only on a new unpublished SSLContext. OpenSSL
    // owns the lookup. Transfer the payload once; lookup free reclaims it.
    let lookup = unsafe { X509_STORE_add_lookup(store.as_ptr(), method) };
    if lookup.is_null() {
        return Err(failure(ErrorStack::get()));
    }
    if !unsafe { X509_LOOKUP_get_method_data(lookup) }.is_null() {
        return Err(invalid("CA directory lookup already installed"));
    }
    let control = DirectoryLookup {
        snapshot: Arc::new(RwLock::new(snapshot)),
        budget: Arc::new(Mutex::new(StoreBudget::default())),
    };
    let data = Box::into_raw(Box::new(control.clone()));
    if unsafe { X509_LOOKUP_set_method_data(lookup, data.cast()) } != 1 {
        unsafe { drop(Box::from_raw(data)) };
        return Err(failure(ErrorStack::get()));
    }
    Ok(control)
}

unsafe extern "C" fn free_snapshot(lookup: *mut Lookup) {
    // SAFETY: OpenSSL calls this once at the final store release. Its context
    // and all SSL connections have already released their store references.
    let data = unsafe { X509_LOOKUP_get_method_data(lookup) }.cast::<DirectoryLookup>();
    if !data.is_null() {
        unsafe {
            X509_LOOKUP_set_method_data(lookup, ptr::null_mut());
            drop(Box::from_raw(data));
        }
    }
}

unsafe extern "C" fn get_by_subject(
    lookup: *mut Lookup,
    kind: libc::c_int,
    name: *const ffi::X509_NAME,
    result: *mut ffi::X509_OBJECT,
) -> libc::c_int {
    // A Rust unwind must never cross OpenSSL's C stack. Misses/parsing failures
    // are ordinary lookup misses; the verifier supplies the TLS error/code.
    let found =
        std::panic::catch_unwind(|| unsafe { find(lookup, kind, name, result) }).unwrap_or(0);
    if found == 0 {
        ErrorStack::get();
    }
    found
}

unsafe fn find(
    lookup: *mut Lookup,
    kind: libc::c_int,
    name: *const ffi::X509_NAME,
    result: *mut ffi::X509_OBJECT,
) -> libc::c_int {
    if name.is_null() || result.is_null() || !matches!(kind, 1 | 2) {
        return 0;
    }
    let data = unsafe { X509_LOOKUP_get_method_data(lookup) }.cast::<DirectoryLookup>();
    let store = unsafe { X509_LOOKUP_get_store(lookup) };
    if data.is_null() || store.is_null() {
        return 0;
    }
    // SAFETY: control and store are retained by the active lookup. Keep the
    // snapshot read lock until all borrowed file bytes have been consumed.
    let control = unsafe { &*data };
    let data = control.snapshot.read().unwrap_or_else(|p| p.into_inner());
    let store_ref = unsafe { X509StoreRef::from_ptr(store) };
    let name_ref = unsafe { openssl::x509::X509NameRef::from_ptr(name.cast_mut()) };
    // The safe subject_name_hash API uses OpenSSL's canonical name hashing
    // across 1.1.1/3.x. Copy the query into a temporary subject-only X509 rather
    // than linking the 3.x-only X509_NAME_hash_ex symbol or hashing raw DER.
    let Ok(mut query) = openssl::x509::X509::builder() else {
        return 0;
    };
    if query.set_subject_name(name_ref).is_err() {
        return 0;
    }
    let hash = query.build().subject_name_hash();
    let postfix = if kind == 2 { "r" } else { "" };
    for directory in &data.directories {
        // Match OpenSSL's contiguous hash.N/hash.rN search, starting at zero.
        for index in 0..=4096 {
            let Some(bytes) = directory.get(&format!("{hash:08x}.{postfix}{index}")) else {
                break;
            };
            let Ok(entries) = crate::tls_store::directory_entries(bytes, kind == 2) else {
                break;
            };
            if control
                .budget
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .add(store_ref, entries)
                .is_err()
            {
                return 0;
            }
        }
        let Ok(_lock) = StoreLock::acquire(store_ref) else {
            return 0;
        };
        let objects = unsafe { ffi::X509_STORE_get0_objects(store) };
        if objects.is_null() {
            return 0;
        }
        let object = unsafe { X509_OBJECT_retrieve_by_subject(objects, kind, name) };
        if object.is_null() {
            continue;
        }
        // set1 increments a reference. The lookup's temporary output is borrowed:
        // X509_STORE_CTX_get_by_subject increments it again for its owned result.
        // Balance set1's extra reference while the native store lock retains the
        // underlying cached object (OpenSSL's documented lookup convention).
        return if kind == 1 {
            let cert = unsafe { ffi::X509_OBJECT_get0_X509(object) };
            let ok = unsafe { X509_OBJECT_set1_X509(result, cert) };
            if ok == 1 {
                unsafe { ffi::X509_free(cert) };
            }
            ok
        } else {
            let crl = unsafe { X509_OBJECT_get0_X509_CRL(object) };
            let ok = unsafe { X509_OBJECT_set1_X509_CRL(result, crl) };
            if ok == 1 {
                unsafe { ffi::X509_CRL_free(crl) };
            }
            ok
        };
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::ssl_bio::{StoreQuery, TrustFile};
    use openssl::{
        stack::Stack,
        x509::{store::X509StoreBuilder, X509StoreContext, X509},
    };

    #[test]
    fn directory_lookup_is_lazy_contiguous_bounded_and_type_specific() {
        let pem = include_bytes!("../../../../tests/fixtures/ssl/server.crt");
        let cert = X509::from_pem(pem).unwrap();
        let hash = cert.subject_name_hash();
        for (suffix, valid) in [("0", true), ("1", false)] {
            let mut builder = X509StoreBuilder::new().unwrap();
            // A malformed unrelated CRL must not poison a cert-only lookup.
            let data = [
                pem.as_slice(),
                b"-----BEGIN X509 CRL-----\ninvalid\n-----END X509 CRL-----\n",
            ]
            .concat();
            install(
                &mut builder,
                vec![TrustDirectory {
                    files: vec![TrustFile {
                        name: format!("{hash:08x}.{suffix}"),
                        data,
                    }],
                }],
            )
            .unwrap();
            let store = builder.build();
            assert_eq!(
                crate::tls_store::details(&store, StoreQuery::Stats)
                    .unwrap()
                    .x509,
                0
            );
            let mut verify = X509StoreContext::new().unwrap();
            let chain = Stack::new().unwrap();
            assert_eq!(
                verify
                    .init(&store, &cert, &chain, |ctx| ctx.verify_cert())
                    .unwrap(),
                valid
            );
            let details = crate::tls_store::details(&store, StoreQuery::Der).unwrap();
            assert_eq!(details.x509, u32::from(valid));
            assert_eq!(details.certificates.len(), usize::from(valid));
        }
        let mut store = X509StoreBuilder::new().unwrap();
        for (name, data) in [
            ("../00000000.0", vec![]),
            ("00000000.00", vec![]),
            ("00000000.0", vec![0; BUFFER_LIMIT]),
        ] {
            assert!(install(
                &mut store,
                vec![TrustDirectory {
                    files: vec![TrustFile {
                        name: name.to_owned(),
                        data,
                    }]
                }]
            )
            .is_err());
        }
        assert!(install(
            &mut store,
            (0..17).map(|_| TrustDirectory { files: vec![] }).collect()
        )
        .is_err());
    }
}
