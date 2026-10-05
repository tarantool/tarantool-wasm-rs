use crate::*;
#[test]
fn pem_decoder_reads_only_first_certificate_and_is_bounded() {
    let pem = include_bytes!("../../../../tests/fixtures/ssl/decoded.crt");
    let expected = tls_certificate::decode_pem(pem).unwrap();
    let mut bundle = pem.to_vec();
    bundle.extend_from_slice(b"\n-----BEGIN CERTIFICATE-----\ninvalid trailing certificate\n");
    assert_eq!(
        tls_certificate::decode_pem(&bundle).unwrap().serial_number,
        expected.serial_number
    );
    assert!(tls_certificate::decode_pem(include_bytes!(
        "../../../../tests/fixtures/ssl/server.key"
    ))
    .is_err());
    assert!(tls_certificate::decode_pem(&vec![b' '; tls_bio::BUFFER_LIMIT + 1]).is_err());
}

#[test]
fn module_helpers_are_bounded_and_use_openssl() {
    use bindings::ssl_bio::Asn1Query;
    assert!(tls_utilities::random_status());
    assert!(tls_utilities::random_bytes(0).unwrap().is_empty());
    assert_eq!(tls_utilities::random_bytes(257).unwrap().len(), 257);
    assert!(tls_utilities::random_bytes(u32::MAX).is_err());
    assert!(tls_utilities::random_add(&vec![0; tls_bio::BUFFER_LIMIT + 1], 0.0).is_err());
    tls_utilities::random_add(b"guest input", 0.0).unwrap();
    let server = tls_utilities::lookup(Asn1Query::Name("serverAuth".into())).unwrap();
    assert_eq!(server.nid, 129);
    assert_eq!(server.oid, "1.3.6.1.5.5.7.3.1");
    let client = tls_utilities::lookup(Asn1Query::Nid(130)).unwrap();
    assert_eq!(client.shortname, "clientAuth");
    for query in [
        Asn1Query::Oid("serverAuth".into()),
        Asn1Query::Oid("1.3.6.1.4.1.55555.1".into()),
        Asn1Query::Nid(0),
        Asn1Query::Nid(i32::MAX),
        Asn1Query::Name("x".repeat(4097)),
        Asn1Query::Name("serverAuth\0".into()),
    ] {
        assert!(tls_utilities::lookup(query).is_err());
    }
    let defaults = tls_utilities::default_verify_paths().unwrap();
    assert_eq!(defaults.cafile_env, "SSL_CERT_FILE");
    assert_eq!(defaults.capath_env, "SSL_CERT_DIR");
    assert!(!defaults.cafile.is_empty());
    assert!(!defaults.capath.is_empty());
}
#[test]
fn symbolic_errors_use_last_code_and_leave_error_queue_untouched() {
    let invalid = openssl::x509::X509::from_der(&[0x31, 0]).unwrap_err();
    let message = invalid.to_string();
    let last = invalid.errors().last().unwrap();
    let details = tls_errors::details(&message);
    assert_eq!(details.library.as_deref(), Some("ASN1"));
    // The last error is a common nested-ASN1 code without a table mnemonic.
    assert!(details.reason.is_none());
    let first = tls_errors::details(&invalid.errors()[0].to_string());
    assert_eq!(first.library.as_deref(), Some("ASN1"));
    assert_eq!(first.reason.as_deref(), Some("WRONG_TAG"));
    let unknown = "error:7F000FFF:unknown:unknown:unknown:file:0:";
    let combined = tls_errors::details(&format!("{unknown}, {message}"));
    assert_eq!(combined.library, details.library);
    assert_eq!(combined.reason, details.reason);
    let final_unknown = tls_errors::details(&format!("{message}, {unknown}"));
    assert!(final_unknown.library.is_none() && final_unknown.reason.is_none());
    for message in [
        "",
        "WANT_READ",
        "error:x:no",
        "error:10000000000000000:no",
        "error:00000000:no",
        "error:123",
        "error::no",
    ] {
        let result = tls_errors::details(message);
        assert!(result.library.is_none() && result.reason.is_none());
    }
    assert!(tls_errors::details(&"x".repeat(tls_bio::BUFFER_LIMIT + 1))
        .library
        .is_none());
    // Reinsert an actual owned error and check the metadata lookup neither
    // reads nor clears native thread-local errors, even with an unknown code.
    last.put();
    let _ = tls_errors::details(&message);
    let _ = tls_errors::verification_message(18);
    assert_eq!(
        openssl::error::ErrorStack::get()
            .errors()
            .last()
            .unwrap()
            .code(),
        last.code()
    );
}

#[test]
fn verification_messages_use_native_literals_and_handle_unknown_codes() {
    assert_eq!(tls_errors::verification_message(0), "ok");
    assert_eq!(
        tls_errors::verification_message(18)
            .to_lowercase()
            .replace('-', " "),
        "self signed certificate"
    );
    assert_eq!(
        tls_errors::verification_message(62).to_lowercase(),
        "hostname mismatch"
    );
    assert_eq!(
        tls_errors::verification_message(64).to_lowercase(),
        "ip address mismatch"
    );
    assert_eq!(tls_errors::verification_message(23), "certificate revoked");
    for code in [-1, i32::MIN, i32::MAX] {
        assert_eq!(
            tls_errors::verification_message(code),
            "unknown certificate verification error"
        );
    }
}
