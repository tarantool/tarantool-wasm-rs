//! Python SSL compatibility host. All native TLS operations are synchronous,
//! memory-only and serialized through the embedding runtime's resource table.
use std::ops::DerefMut;
use wasmtime::component::{Linker, ResourceTable};

pub mod bindings;
mod host;
mod tls_alpn;
mod tls_bio;
mod tls_capath;
mod tls_certificate;
mod tls_connection;
mod tls_credentials;
mod tls_errors;
mod tls_key_exchange;
mod tls_keylog;
mod tls_messages;
mod tls_pem;
mod tls_policy;
mod tls_post_handshake;
mod tls_protocols;
mod tls_session;
mod tls_sni;
mod tls_store;
mod tls_transport;
mod tls_utilities;

/// Access to the same table used by WASI and the embedding runtime.
/// The guard must serialize all SSL operations; no operation awaits while held.
pub trait SslView: Send {
    type Table<'a>: DerefMut<Target = ResourceTable>
    where
        Self: 'a;
    fn ssl_table(&mut self) -> Self::Table<'_>;
}

pub fn add_to_linker<T: SslView + 'static>(linker: &mut Linker<T>) -> wasmtime::Result<()> {
    bindings::ssl_bio::add_to_linker::<T, SslHostData<T>>(linker, |state| SslHost(state))
}

#[cfg(test)]
mod regression;
#[cfg(test)]
#[path = "../build_support/ssl_error_tables.rs"]
mod ssl_error_tables;
#[cfg(test)]
#[path = "../build_support/ssl_version.rs"]
mod ssl_version;

struct SslHost<'a, T>(&'a mut T);
struct SslHostData<T>(std::marker::PhantomData<T>);
impl<T: SslView + 'static> wasmtime::component::HasData for SslHostData<T> {
    type Data<'a> = SslHost<'a, T>;
}
