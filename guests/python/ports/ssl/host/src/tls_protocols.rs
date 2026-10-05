//! Protocol availability comes from the linked OpenSSL's configuration headers.
include!(concat!(env!("OUT_DIR"), "/ssl_protocols.rs"));

pub fn capabilities() -> Vec<u16> {
    COMPILED_PROTOCOLS.to_vec()
}

/// Header version used for this native module, independent of the loaded library.
pub fn compiled_api_version() -> u64 {
    COMPILED_API_VERSION
}
