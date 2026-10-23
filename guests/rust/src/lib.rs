//! Synchronous Tarantool guest API. Host calls require a Tarantool Wasm instance.
//!
//! ```no_run
//! use tarantool_wasm::{Space, Result};
//!
//! fn create_user() -> Result<()> {
//!     let users = Space::by_name("users")?.expect("schema must exist");
//!     let row = users.insert(&(1u64, "Alice"))?;
//!     let user: (u64, String) = row.decode()?;
//!     assert_eq!(user.0, 1);
//!     Ok(())
//! }
//! ```
mod database;
pub mod msgpack;

/// Complete released platform API. Raw record handles require explicit release.
pub mod bindings {
    wit_bindgen::generate!({ path: "wit", world: "bindings", generate_all });
}

pub use bindings::tarantool::tarantool as raw;
pub use database::{Index, Scan, Space, Tuple};
pub use raw::types::{BoxError, IteratorType};
pub use rmpv::Value;

/// Host errors retain all fields, including unknown numeric error codes.
#[derive(Debug)]
pub enum Error {
    Host(BoxError),
    Encode(rmp_serde::encode::Error),
    Decode(rmp_serde::decode::Error),
    Invalid(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Host(e) => write!(f, "{} ({}): {}", e.error_type, e.code, e.message),
            Self::Encode(e) => e.fmt(f),
            Self::Decode(e) => e.fmt(f),
            Self::Invalid(e) => f.write_str(e),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Host(e) => Some(e),
            Self::Encode(e) => Some(e),
            Self::Decode(e) => Some(e),
            Self::Invalid(_) => None,
        }
    }
}

impl From<BoxError> for Error {
    fn from(e: BoxError) -> Self {
        Self::Host(e)
    }
}

impl From<rmp_serde::encode::Error> for Error {
    fn from(e: rmp_serde::encode::Error) -> Self {
        Self::Encode(e)
    }
}

impl From<rmp_serde::decode::Error> for Error {
    fn from(e: rmp_serde::decode::Error) -> Self {
        Self::Decode(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
