//! Python 3.12 metadata formatting over OpenSSL-parsed X509 values.
//! Decoding never opens files, fetches AIA/CRL URLs, or changes verification.
use crate::bindings::ssl_bio::{
    AlternativeName, AlternativeNameValue, CertificateAttribute, CertificateDetails,
    CertificateNameValue, DistinguishedName, TlsError,
};
use crate::tls_bio::{failure, invalid, BUFFER_LIMIT};
use foreign_types::ForeignTypeRef;
use openssl::{
    asn1::Asn1ObjectRef,
    nid::Nid,
    x509::{GeneralNameRef, X509NameRef, X509Ref, X509},
};
use openssl_sys as ffi;
use std::ptr::NonNull;

// Stable public accessors not exposed by openssl-sys. The ABI matches
// OpenSSL's x509.h, x509v3.h and bio.h (OpenSSL 1.1.1 and 3.x).
unsafe extern "C" {
    fn X509_NAME_ENTRY_set(entry: *const ffi::X509_NAME_ENTRY) -> libc::c_int;
    fn GENERAL_NAME_print(bio: *mut ffi::BIO, name: *const ffi::GENERAL_NAME) -> libc::c_int;
    fn BIO_gets(bio: *mut ffi::BIO, buf: *mut libc::c_char, size: libc::c_int) -> libc::c_int;
    fn X509_NAME_print_ex(
        bio: *mut ffi::BIO,
        name: *const ffi::X509_NAME,
        indent: libc::c_int,
        flags: libc::c_ulong,
    ) -> libc::c_int;
    fn PEM_write_bio_X509_AUX(bio: *mut ffi::BIO, cert: *const ffi::X509) -> libc::c_int;
}

/// Each value owns one X509 reference; it survives SSL/context destruction.
pub struct BioCertificate(pub X509);

impl BioCertificate {
    pub fn info(&self) -> Result<CertificateDetails, TlsError> {
        decode(&self.0)
    }
    pub fn subject_hash(&self) -> u32 {
        self.0.subject_name_hash()
    }
    pub fn equal(&self, other: &Self) -> bool {
        self.0 == other.0
    }
    pub fn public_bytes(&self, format: u32) -> Result<Vec<u8>, TlsError> {
        let bytes = match format {
            1 => self.0.to_pem().map_err(failure)?,
            2 => self.0.to_der().map_err(failure)?,
            0x101 => {
                let bio = MemoryBio::new()?;
                // SAFETY: live owned memory BIO and borrowed X509. No pointer
                // is retained; AUX trust data is serialized only to this BIO.
                if unsafe { PEM_write_bio_X509_AUX(bio.0.as_ptr(), self.0.as_ptr()) } != 1 {
                    return Err(failure(openssl::error::ErrorStack::get()));
                }
                bio.bytes()?
            }
            _ => return Err(invalid("Unsupported format")),
        };
        if bytes.len() > BUFFER_LIMIT {
            return Err(invalid("certificate output limit exceeded"));
        }
        Ok(bytes)
    }
    pub fn subject(&self) -> Result<String, TlsError> {
        let bio = MemoryBio::new()?;
        // XN_FLAG_RFC2253, from public x509.h: escape ASN1 strings, reverse
        // RDN order and use short names. Matches CPython Certificate.__repr__.
        const RFC2253: libc::c_ulong = 0x1110317;
        // SAFETY: both borrowed native values outlive this synchronous print.
        if unsafe { X509_NAME_print_ex(bio.0.as_ptr(), self.0.subject_name().as_ptr(), 0, RFC2253) }
            <= 0
        {
            return Err(failure(openssl::error::ErrorStack::get()));
        }
        String::from_utf8(bio.bytes()?).map_err(failure)
    }
}

pub fn decode_pem(data: &[u8]) -> Result<CertificateDetails, TlsError> {
    let certificate = crate::tls_pem::first_certificate(data)?;
    decode(&certificate)
}

fn oid_text(object: &Asn1ObjectRef) -> Result<String, TlsError> {
    // SAFETY: object borrows a live OpenSSL value. The first call queries the
    // output size, and the second uses an owned length+1 byte buffer.
    let length = unsafe { ffi::OBJ_obj2txt(std::ptr::null_mut(), 0, object.as_ptr(), 0) };
    if length <= 0 || length as usize >= BUFFER_LIMIT {
        return Err(failure("invalid certificate attribute OID"));
    }
    let mut bytes = vec![0u8; length as usize + 1];
    let written =
        unsafe { ffi::OBJ_obj2txt(bytes.as_mut_ptr().cast(), length + 1, object.as_ptr(), 0) };
    if written != length {
        return Err(failure("certificate attribute OID formatting failed"));
    }
    bytes.truncate(length as usize);
    String::from_utf8(bytes).map_err(failure)
}

fn distinguished_name(name: &X509NameRef) -> Result<DistinguishedName, TlsError> {
    let mut result: DistinguishedName = Vec::new();
    let mut previous_set = None;
    for entry in name.entries() {
        // SAFETY: the entry and its ASN1 string remain owned by the borrowed
        // name. These accessors only read the RDN index and the string tag.
        let set = unsafe { X509_NAME_ENTRY_set(entry.as_ptr()) };
        if previous_set != Some(set) {
            result.push(Vec::new());
            previous_set = Some(set);
        }
        let value =
            if unsafe { ffi::ASN1_STRING_type(entry.data().as_ptr()) } == ffi::V_ASN1_BIT_STRING {
                CertificateNameValue::Binary(entry.data().as_slice().to_vec())
            } else {
                // Unlike deprecated as_utf8(), this preserves embedded NULs.
                CertificateNameValue::Text(entry.data().to_string().map_err(failure)?)
            };
        result.last_mut().unwrap().push(CertificateAttribute {
            name: oid_text(entry.object())?,
            value,
        });
    }
    Ok(result)
}

struct MemoryBio(NonNull<ffi::BIO>);
impl MemoryBio {
    fn new() -> Result<Self, TlsError> {
        // SAFETY: OpenSSL is initialized by the owning X509/SSL resource.
        // A memory BIO owns no file/socket. Drop releases the allocation.
        NonNull::new(unsafe { ffi::BIO_new(ffi::BIO_s_mem()) })
            .map(Self)
            .ok_or_else(|| failure(openssl::error::ErrorStack::get()))
    }
    fn first_line(&self) -> Result<String, TlsError> {
        // CPython uses a bounded GENERAL_NAME_print/BIO_gets fallback. Common
        // DNS/email/URI values use explicit ASN1 lengths instead of this path.
        let mut bytes = [0u8; 2048];
        // SAFETY: this live owned BIO writes at most size-1 bytes into bytes.
        let length = unsafe { BIO_gets(self.0.as_ptr(), bytes.as_mut_ptr().cast(), 2047) };
        if length < 0 {
            return Err(failure(openssl::error::ErrorStack::get()));
        }
        String::from_utf8(bytes[..length as usize].to_vec()).map_err(failure)
    }
    fn bytes(&self) -> Result<Vec<u8>, TlsError> {
        // SAFETY: query and drain this uniquely owned memory BIO. No native
        // pointer escapes. Reject oversized output before allocating a copy.
        let size = unsafe {
            ffi::BIO_ctrl(
                self.0.as_ptr(),
                10, // BIO_CTRL_PENDING, public bio.h control number.
                0,
                std::ptr::null_mut(),
            )
        };
        if size < 0 || size as usize > BUFFER_LIMIT {
            return Err(invalid("certificate output limit exceeded"));
        }
        let mut bytes = vec![0; size as usize];
        if size != 0
            && unsafe {
                ffi::BIO_read(
                    self.0.as_ptr(),
                    bytes.as_mut_ptr().cast(),
                    size as libc::c_int,
                )
            } != size as libc::c_int
        {
            return Err(failure("certificate BIO read failed"));
        }
        Ok(bytes)
    }
}
impl Drop for MemoryBio {
    fn drop(&mut self) {
        // SAFETY: BIO_new allocated this pointer; it is owned only here and
        // freed once. It has no chained or borrowed BIOs.
        unsafe { ffi::BIO_free_all(self.0.as_ptr()) };
    }
}

fn alternative_name(name: &GeneralNameRef) -> Result<AlternativeName, TlsError> {
    if let Some(value) = name.directory_name() {
        return Ok(AlternativeName {
            kind: "DirName".into(),
            value: AlternativeNameValue::Name(distinguished_name(value)?),
        });
    }
    for (kind, value) in [
        ("DNS", name.dnsname()),
        ("email", name.email()),
        ("URI", name.uri()),
    ] {
        if let Some(value) = value {
            return Ok(AlternativeName {
                kind: kind.into(),
                value: AlternativeNameValue::Text(value.to_owned()),
            });
        }
    }
    if let Some(bytes) = name.ipaddress() {
        let text = match bytes {
            [a, b, c, d] => format!("{a}.{b}.{c}.{d}"),
            bytes if bytes.len() == 16 => bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|chunk| format!("{:X}", u16::from_be_bytes(*chunk)))
                .collect::<Vec<_>>()
                .join(":"),
            _ => "<invalid>".into(),
        };
        return Ok(AlternativeName {
            kind: "IP Address".into(),
            value: AlternativeNameValue::Text(text),
        });
    }
    // SAFETY: GeneralNameRef borrows a live OpenSSL-decoded GENERAL_NAME. The
    // tag identifies the pointer's type; from_ptr only borrows that value.
    if unsafe { (*name.as_ptr()).type_ } == ffi::GEN_RID {
        let object = unsafe { (*name.as_ptr()).d };
        if object.is_null() {
            return Err(failure("missing registered ID"));
        }
        let object = unsafe { Asn1ObjectRef::from_ptr(object.cast()) };
        let text = oid_text(object)?;
        return Ok(AlternativeName {
            kind: "Registered ID".into(),
            value: AlternativeNameValue::Text(if text.len() >= 2048 {
                "<INVALID>".into()
            } else {
                text
            }),
        });
    }
    // Invalid UTF-8 in an IA5 SAN must fail, not silently fall back to a print
    // function which can truncate at an embedded NUL.
    if matches!(
        unsafe { (*name.as_ptr()).type_ },
        ffi::GEN_DNS | ffi::GEN_EMAIL | ffi::GEN_URI
    ) {
        return Err(failure("invalid UTF-8 certificate alternative name"));
    }
    let bio = MemoryBio::new()?;
    // SAFETY: both pointers are live borrowed/owned values. Printing appends
    // only to the memory BIO and does not transfer ownership of the name.
    if unsafe { GENERAL_NAME_print(bio.0.as_ptr(), name.as_ptr()) } <= 0 {
        return Err(failure(openssl::error::ErrorStack::get()));
    }
    let text = bio.first_line()?;
    let (kind, value) = text
        .split_once(':')
        .ok_or_else(|| failure("invalid certificate alternative name"))?;
    Ok(AlternativeName {
        kind: kind.into(),
        value: AlternativeNameValue::Text(value.into()),
    })
}

fn nonempty<T>(values: Vec<T>) -> Option<Vec<T>> {
    (!values.is_empty()).then_some(values)
}

fn uri(name: &GeneralNameRef) -> Result<Option<&str>, TlsError> {
    // SAFETY: the borrowed GeneralNameRef owns a live tagged OpenSSL value.
    if unsafe { (*name.as_ptr()).type_ } != ffi::GEN_URI {
        return Ok(None);
    }
    name.uri()
        .map(Some)
        .ok_or_else(|| failure("invalid UTF-8 certificate URI"))
}

pub fn decode(cert: &X509Ref) -> Result<CertificateDetails, TlsError> {
    let subject_alt_name = cert
        .subject_alt_names()
        .map(|names| names.iter().map(alternative_name).collect())
        .transpose()?;
    let mut ocsp = Vec::new();
    let mut ca_issuers = Vec::new();
    if let Some(descriptions) = cert.authority_info() {
        for description in descriptions {
            if let Some(uri) = uri(description.location())? {
                if description.method().nid() == Nid::AD_OCSP {
                    ocsp.push(uri.to_owned());
                } else if description.method().nid() == Nid::AD_CA_ISSUERS {
                    ca_issuers.push(uri.to_owned());
                }
            }
        }
    }
    let mut crl = Vec::new();
    if let Some(points) = cert.crl_distribution_points() {
        for point in points {
            if let Some(names) = point.distpoint().and_then(|name| name.fullname()) {
                for name in names {
                    if let Some(uri) = uri(name)? {
                        crl.push(uri.to_owned());
                    }
                }
            }
        }
    }
    let serial = cert
        .serial_number()
        .to_bn()
        .and_then(|bn| bn.to_hex_str())
        .map_err(failure)?
        .to_string();
    Ok(CertificateDetails {
        subject: distinguished_name(cert.subject_name())?,
        issuer: distinguished_name(cert.issuer_name())?,
        version: u32::try_from(cert.version() + 1).map_err(failure)?,
        serial_number: if serial == "0" { "00".into() } else { serial },
        not_before: cert.not_before().to_string(),
        not_after: cert.not_after().to_string(),
        subject_alt_name,
        ocsp: nonempty(ocsp),
        ca_issuers: nonempty(ca_issuers),
        crl_distribution_points: nonempty(crl),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openssl::x509::{X509NameBuilder, X509};

    #[test]
    fn complex_python_certificate_metadata() {
        let cert =
            X509::from_pem(include_bytes!("../../../../tests/fixtures/ssl/decoded.crt")).unwrap();
        let result = decode(&cert).unwrap();
        assert_eq!(result.version, 3);
        assert_eq!(result.serial_number, "80");
        assert_eq!(result.subject[2].len(), 2);
        assert!(
            matches!(&result.subject[1][0].value, CertificateNameValue::Text(text) if text == "TLS сертификат")
        );
        assert_eq!(result.subject[4][0].name, "1.2.3.4.5.6.7.8.9.10.11.12.13.14.15.16.17.18.19.20.21.22.23.24.25.26.27.28.29.30.31.32.33.34.35.36.37.38.39.40");
        let names = result.subject_alt_name.unwrap();
        assert_eq!(names.len(), 9);
        assert!(
            matches!(&names[3].value, AlternativeNameValue::Text(text) if text == "2001:DB8:0:0:0:0:0:ABCD")
        );
        assert!(matches!(&names[6].value, AlternativeNameValue::Name(name) if name.len() == 3));
        assert_eq!(names[8].kind, "othername");
        assert!(
            matches!(&names[8].value, AlternativeNameValue::Text(text) if text == "<unsupported>")
        );
        assert_eq!(result.ocsp.unwrap().len(), 2);
        assert_eq!(
            result.ca_issuers.unwrap(),
            ["http://ca.example.test/cert.pem"]
        );
        assert_eq!(result.crl_distribution_points.unwrap().len(), 2);
    }

    #[test]
    fn name_preserves_embedded_nul() {
        let mut name = X509NameBuilder::new().unwrap();
        name.append_entry_by_text("CN", "left\0right").unwrap();
        let result = distinguished_name(&name.build()).unwrap();
        assert!(
            matches!(&result[0][0].value, CertificateNameValue::Text(text) if text == "left\0right")
        );
    }
}
