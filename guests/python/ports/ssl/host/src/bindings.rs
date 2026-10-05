wasmtime::component::bindgen!({
    path: "../wit",
    world: "ssl-host",
    imports: { default: async },
    with: {
        "docs:adder/ssl-bio.context": crate::tls_bio::BioContext,
        "docs:adder/ssl-bio.connection": crate::tls_bio::BioConnection,
        "docs:adder/ssl-bio.session": crate::tls_session::BioSession,
        "docs:adder/ssl-bio.certificate": crate::tls_certificate::BioCertificate,
    }
});
pub use docs::adder::ssl_bio;
