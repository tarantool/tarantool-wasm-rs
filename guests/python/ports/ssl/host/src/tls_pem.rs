//! Bounded public PEM parsing with an explicit noninteractive password callback.
use crate::bindings::ssl_bio::TlsError;
use crate::tls_bio::{failure, invalid, BUFFER_LIMIT};
use foreign_types::ForeignType;
use openssl::{
    dh::Dh,
    error::ErrorStack,
    pkey::Params,
    x509::{X509Crl, X509},
};
use openssl_sys as ffi;
use std::{marker::PhantomData, ptr::NonNull};

unsafe extern "C" {
    fn PEM_read_bio_X509_AUX(
        bio: *mut ffi::BIO,
        out: *mut *mut ffi::X509,
        callback: ffi::pem_password_cb,
        userdata: *mut libc::c_void,
    ) -> *mut ffi::X509;
}

struct MemoryBio<'a> {
    pointer: NonNull<ffi::BIO>,
    data: PhantomData<&'a [u8]>,
}
impl<'a> MemoryBio<'a> {
    fn new(data: &'a [u8]) -> Result<Self, TlsError> {
        if data.len() > BUFFER_LIMIT {
            return Err(invalid("TLS PEM input limit exceeded"));
        }
        openssl::init();
        // SAFETY: the bounded immutable slice outlives this borrowing BIO.
        let pointer = NonNull::new(unsafe {
            ffi::BIO_new_mem_buf(data.as_ptr().cast(), data.len() as libc::c_int)
        })
        .ok_or_else(|| failure(ErrorStack::get()))?;
        Ok(Self {
            pointer,
            data: PhantomData,
        })
    }
}
impl Drop for MemoryBio<'_> {
    fn drop(&mut self) {
        // SAFETY: this guard owns one BIO allocation with no chained BIOs.
        unsafe { ffi::BIO_free_all(self.pointer.as_ptr()) };
    }
}

#[cfg(test)]
thread_local! { static REJECTED_PASSWORDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

unsafe extern "C" fn no_password(
    _: *mut libc::c_char,
    _: libc::c_int,
    _: libc::c_int,
    _: *mut libc::c_void,
) -> libc::c_int {
    #[cfg(test)]
    REJECTED_PASSWORDS.with(|count| count.set(count.get() + 1));
    0
}

pub fn certificates(data: &[u8]) -> Result<Vec<X509>, TlsError> {
    let bio = MemoryBio::new(data)?;
    let mut certificates = Vec::new();
    loop {
        // SAFETY: the BIO is live, callbacks cannot prompt, and each successful
        // read returns a new owned X509 with no output-pointer reuse.
        let pointer = unsafe {
            ffi::PEM_read_bio_X509(
                bio.pointer.as_ptr(),
                std::ptr::null_mut(),
                Some(no_password),
                std::ptr::null_mut(),
            )
        };
        if pointer.is_null() {
            let errors = ErrorStack::get();
            if errors.errors().last().is_some_and(|error| {
                error.library_code() == ffi::ERR_LIB_PEM as libc::c_int
                    && error.reason_code() == ffi::PEM_R_NO_START_LINE as libc::c_int
            }) {
                return Ok(certificates);
            }
            return Err(failure(errors));
        }
        certificates.push(unsafe { X509::from_ptr(pointer) });
    }
}

pub fn first_certificate(data: &[u8]) -> Result<X509, TlsError> {
    let bio = MemoryBio::new(data)?;
    // SAFETY: live bounded BIO and noninteractive callback. Read once, as
    // CPython's test decoder does; trailing data is deliberately ignored.
    let pointer = unsafe {
        ffi::PEM_read_bio_X509(
            bio.pointer.as_ptr(),
            std::ptr::null_mut(),
            Some(no_password),
            std::ptr::null_mut(),
        )
    };
    if pointer.is_null() {
        return Err(failure(ErrorStack::get()));
    }
    Ok(unsafe { X509::from_ptr(pointer) })
}

pub fn trusted_certificate(data: &[u8]) -> Result<X509, TlsError> {
    let bio = MemoryBio::new(data)?;
    // SAFETY: live BIO, noninteractive callback; a successful pointer is a new
    // owned X509 retaining AUX data. No borrowed output pointer escapes.
    let pointer = unsafe {
        PEM_read_bio_X509_AUX(
            bio.pointer.as_ptr(),
            std::ptr::null_mut(),
            Some(no_password),
            std::ptr::null_mut(),
        )
    };
    if pointer.is_null() {
        return Err(failure(ErrorStack::get()));
    }
    Ok(unsafe { X509::from_ptr(pointer) })
}

pub fn crl(data: &[u8]) -> Result<X509Crl, TlsError> {
    let bio = MemoryBio::new(data)?;
    // SAFETY: live bounded BIO and explicit no-password callback. Successful
    // PEM_read_bio_X509_CRL returns a fresh allocation owned by the Rust value.
    let pointer = unsafe {
        ffi::PEM_read_bio_X509_CRL(
            bio.pointer.as_ptr(),
            std::ptr::null_mut(),
            Some(no_password),
            std::ptr::null_mut(),
        )
    };
    if pointer.is_null() {
        return Err(failure(ErrorStack::get()));
    }
    Ok(unsafe { X509Crl::from_ptr(pointer) })
}

pub fn dh(data: &[u8]) -> Result<Dh<Params>, TlsError> {
    let bio = MemoryBio::new(data)?;
    // SAFETY: live bounded BIO and explicit no-password callback. Successful
    // PEM_read_bio_DHparams returns fresh public parameters owned by this value.
    let pointer = unsafe {
        ffi::PEM_read_bio_DHparams(
            bio.pointer.as_ptr(),
            std::ptr::null_mut(),
            Some(no_password),
            std::ptr::null_mut(),
        )
    };
    if pointer.is_null() {
        return Err(failure(ErrorStack::get()));
    }
    Ok(unsafe { Dh::from_ptr(pointer) })
}

#[cfg(test)]
mod tests {
    use super::*;
    type Reader = fn(&[u8]) -> Result<(), TlsError>;
    fn encrypted(label: &str) -> Vec<u8> {
        format!("-----BEGIN {label}-----\nProc-Type: 4,ENCRYPTED\nDEK-Info: AES-256-CBC,00000000000000000000000000000000\n\nAAAAAAAAAAAAAAAAAAAAAA==\n-----END {label}-----\n").into_bytes()
    }
    #[test]
    fn encrypted_public_objects_explicitly_reject_password_requests() {
        let cases: [(&str, Reader); 4] = [
            ("CERTIFICATE", |data| certificates(data).map(|_| ())),
            ("TRUSTED CERTIFICATE", |data| {
                trusted_certificate(data).map(|_| ())
            }),
            ("X509 CRL", |data| crl(data).map(|_| ())),
            ("DH PARAMETERS", |data| dh(data).map(|_| ())),
        ];
        for (label, read) in cases {
            let before = REJECTED_PASSWORDS.with(|count| count.get());
            assert!(read(&encrypted(label)).is_err());
            assert_eq!(
                REJECTED_PASSWORDS.with(|count| count.get()),
                before + 1,
                "{label}"
            );
        }
    }
    #[test]
    fn all_public_readers_bound_input_before_openssl() {
        let data = vec![0; BUFFER_LIMIT + 1];
        assert!(certificates(&data).is_err());
        assert!(trusted_certificate(&data).is_err());
        assert!(crl(&data).is_err());
        assert!(dh(&data).is_err());
    }
}
