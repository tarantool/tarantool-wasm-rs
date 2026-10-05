//! Bounded OpenSSL module helpers. No caller-supplied host paths or I/O.
use crate::bindings::ssl_bio::{Asn1ObjectInfo, Asn1Query, TlsError, VerifyPathDefaults};
use crate::tls_bio::{failure, invalid, BUFFER_LIMIT};
use foreign_types::{ForeignType, ForeignTypeRef};
use openssl::asn1::{Asn1Object, Asn1ObjectRef};
use openssl_sys as ffi;
use std::ffi::{CStr, CString};

pub fn random_bytes(size: u32) -> Result<Vec<u8>, TlsError> {
    let size = size as usize;
    if size > BUFFER_LIMIT {
        return Err(invalid("random byte transfer exceeds 1 MiB"));
    }
    let mut bytes = vec![0; size];
    openssl::rand::rand_bytes(&mut bytes).map_err(failure)?;
    Ok(bytes)
}

pub fn random_status() -> bool {
    openssl::init();
    // SAFETY: process-global initialized OpenSSL PRNG, no borrowed pointers.
    unsafe { ffi::RAND_status() == 1 }
}

pub fn random_add(data: &[u8], entropy: f64) -> Result<(), TlsError> {
    if data.len() > BUFFER_LIMIT {
        return Err(invalid("random byte transfer exceeds 1 MiB"));
    }
    openssl::init();
    // SAFETY: input is borrowed only for this synchronous call. The 1 MiB
    // bound fits c_int. OpenSSL copies/mixes the bytes and retains no pointer.
    unsafe { ffi::RAND_add(data.as_ptr().cast(), data.len() as libc::c_int, entropy) };
    Ok(())
}

fn object_info(object: &Asn1ObjectRef) -> Result<Asn1ObjectInfo, TlsError> {
    let nid = object.nid();
    if nid.as_raw() <= 0 {
        return Err(invalid("Unknown object"));
    }
    let shortname = nid.short_name().map_err(|_| invalid("Unknown object"))?;
    let longname = nid.long_name().map_err(|_| invalid("Unknown object"))?;
    // SAFETY: object is live for both calls; output is numeric OID text, with
    // a queried size and an owned size+1 buffer. Neither call takes ownership.
    let size = unsafe { ffi::OBJ_obj2txt(std::ptr::null_mut(), 0, object.as_ptr(), 1) };
    if size <= 0 || size as usize >= BUFFER_LIMIT {
        return Err(invalid("Unknown object"));
    }
    let mut bytes = vec![0u8; size as usize + 1];
    if unsafe { ffi::OBJ_obj2txt(bytes.as_mut_ptr().cast(), size + 1, object.as_ptr(), 1) } != size
    {
        return Err(failure("OID formatting failed"));
    }
    bytes.truncate(size as usize);
    Ok(Asn1ObjectInfo {
        nid: nid.as_raw(),
        shortname: shortname.to_owned(),
        longname: longname.to_owned(),
        oid: String::from_utf8(bytes).map_err(failure)?,
    })
}

pub fn lookup(query: Asn1Query) -> Result<Asn1ObjectInfo, TlsError> {
    openssl::init();
    let numeric_only = matches!(&query, Asn1Query::Oid(_));
    match query {
        Asn1Query::Nid(nid) => {
            if nid < 0 {
                return Err(invalid("NID must be positive."));
            }
            // SAFETY: OBJ_nid2obj returns a borrowed static object or null.
            // The ref is used synchronously and never freed or exported.
            let object = unsafe { ffi::OBJ_nid2obj(nid) };
            if object.is_null() {
                return Err(invalid(format!("unknown NID {nid}")));
            }
            object_info(unsafe { Asn1ObjectRef::from_ptr(object) })
        }
        Asn1Query::Oid(text) | Asn1Query::Name(text) => {
            if text.len() > 4096 {
                return Err(invalid("OID lookup text exceeds 4096 bytes"));
            }
            let text = CString::new(text).map_err(|_| invalid("embedded null character"))?;
            // SAFETY: terminated input is bounded and borrowed for the call.
            // The result is owned by Asn1Object's RAII wrapper, including on
            // unknown objects/error paths. OpenSSL's registry is not modified.
            let object = unsafe { ffi::OBJ_txt2obj(text.as_ptr(), i32::from(numeric_only)) };
            if object.is_null() {
                openssl::error::ErrorStack::get();
                return Err(invalid("Unknown object"));
            }
            let object = unsafe { Asn1Object::from_ptr(object) };
            object_info(&object)
        }
    }
}

pub fn default_verify_paths() -> Result<VerifyPathDefaults, TlsError> {
    openssl::init();
    // SAFETY: OpenSSL returns static NUL-terminated compiled defaults. Copy
    // them immediately; no environment is inspected and no paths are opened.
    unsafe fn bytes(pointer: *const libc::c_char) -> Result<Vec<u8>, TlsError> {
        if pointer.is_null() {
            return Err(failure("missing OpenSSL default path"));
        }
        Ok(unsafe { CStr::from_ptr(pointer) }.to_bytes().to_vec())
    }
    unsafe {
        Ok(VerifyPathDefaults {
            cafile_env: String::from_utf8(bytes(ffi::X509_get_default_cert_file_env())?)
                .map_err(failure)?,
            cafile: bytes(ffi::X509_get_default_cert_file())?,
            capath_env: String::from_utf8(bytes(ffi::X509_get_default_cert_dir_env())?)
                .map_err(failure)?,
            capath: bytes(ffi::X509_get_default_cert_dir())?,
        })
    }
}
