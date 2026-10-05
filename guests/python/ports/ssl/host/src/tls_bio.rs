//! OpenSSL over bounded in-memory transport; no host files, sockets or
//! Tarantool state are accessed. The guest drives transport and readiness.

use crate::bindings::ssl_bio::{
    Config, ConnectionInfo, ErrorKind, PeerCertificate, TlsError, TlsVersion, VerifyMode,
};
use foreign_types::{ForeignType, ForeignTypeRef};
use openssl::ssl::{
    ErrorCode, Ssl, SslContext, SslContextBuilder, SslMethod, SslMode, SslOptions, SslStream,
    SslVerifyMode, SslVersion,
};
use std::{
    collections::{HashMap, VecDeque},
    io::{self, Read, Write},
    net::IpAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

pub const BUFFER_LIMIT: usize = 1024 * 1024;

unsafe extern "C" {
    fn X509_VERIFY_PARAM_get_hostflags(
        param: *const openssl_sys::X509_VERIFY_PARAM,
    ) -> std::ffi::c_uint;
}

pub fn invalid(message: impl Into<String>) -> TlsError {
    TlsError {
        kind: ErrorKind::InvalidInput,
        message: message.into(),
        verify_code: None,
    }
}
pub(super) fn failure(error: impl std::fmt::Display) -> TlsError {
    TlsError {
        kind: ErrorKind::Failure,
        message: error.to_string(),
        verify_code: None,
    }
}
fn version(value: TlsVersion) -> SslVersion {
    match value {
        TlsVersion::Tls12 => SslVersion::TLS1_2,
        TlsVersion::Tls13 => SslVersion::TLS1_3,
    }
}

pub struct BioContext {
    inner: SslContext,
    server: bool,
    check_hostname: bool,
    hostname_common_name: bool,
    sni: crate::tls_sni::Policy,
    directories: Option<crate::tls_capath::DirectoryLookup>,
    post_handshake_auth: bool,
    alpn: crate::tls_alpn::Policy,
    credential_sizes: HashMap<u32, usize>,
}

impl BioContext {
    /// Replace a certificate/key slot without replacing this context.
    ///
    /// # Safety
    /// Caller must exclude every associated SSL/SSL_CTX operation.
    pub unsafe fn update_credentials(
        &mut self,
        value: crate::bindings::ssl_bio::Credentials,
    ) -> Result<(), TlsError> {
        let parsed = crate::tls_credentials::parse(&value)?;
        let previous = self
            .credential_sizes
            .get(&parsed.key_type)
            .copied()
            .unwrap_or(0);
        let retained = self.credential_sizes.values().sum::<usize>() - previous + parsed.size;
        if retained > BUFFER_LIMIT
            || (!self.credential_sizes.contains_key(&parsed.key_type)
                && self.credential_sizes.len() >= 16)
        {
            return Err(invalid("retained TLS credential limit exceeded"));
        }
        // SAFETY: forwarded caller contract. Public atomic replacement retains
        // input objects before their Rust owners are dropped at return.
        unsafe { parsed.install(&self.inner) }?;
        self.credential_sizes.insert(parsed.key_type, parsed.size);
        Ok(())
    }
    pub fn update_sni(&self, enabled: bool) {
        self.sni.update(enabled);
    }
    /// Update ALPN defaults/server preference, retaining native context state.
    ///
    /// # Safety
    /// Caller must exclude every associated SSL/SSL_CTX operation.
    pub unsafe fn update_alpn(&self, protocols: Vec<String>) -> Result<(), TlsError> {
        // SAFETY: forwarded serialization contract, no ownership changes.
        unsafe { self.alpn.update(&self.inner, protocols) }
    }
    /// Update bounded binary ALPN on this retained native context.
    ///
    /// # Safety
    /// Caller must exclude every associated SSL/SSL_CTX operation.
    pub unsafe fn update_alpn_wire(&self, wire: Vec<u8>) -> Result<(), TlsError> {
        // SAFETY: forwarded serialization contract; setter copies the input.
        unsafe { self.alpn.update_wire(&self.inner, wire) }
    }
    /// Change hostname flags copied by SSL_new into future connections.
    ///
    /// # Safety
    /// Caller must exclude every associated SSL/SSL_CTX operation.
    pub unsafe fn update_host_flags(&self, flags: u32) -> Result<(), TlsError> {
        // SAFETY: public borrowed parameter belongs to the live SSL_CTX;
        // native mutation is serialized and no pointer escapes this call.
        unsafe {
            let param = openssl_sys::SSL_CTX_get0_param(self.inner.as_ptr());
            if param.is_null() {
                return Err(failure("missing TLS verification parameters"));
            }
            openssl_sys::X509_VERIFY_PARAM_set_hostflags(param, flags);
        }
        Ok(())
    }
    /// Change DH or ECDH settings on this same native context.
    ///
    /// # Safety
    /// Caller must exclude every associated SSL/SSL_CTX operation.
    pub unsafe fn update_key_exchange(
        &self,
        settings: crate::bindings::ssl_bio::KeyExchangeUpdate,
    ) -> Result<(), TlsError> {
        // SAFETY: forwarded serialization contract, no ownership changes.
        unsafe { crate::tls_key_exchange::update_shared(&self.inner, settings) }
    }
    /// Change TLS 1.3 defaults while retaining the native context.
    ///
    /// # Safety
    /// Caller must exclude every associated SSL/SSL_CTX operation.
    pub unsafe fn update_tls13(
        &mut self,
        settings: crate::bindings::ssl_bio::Tls13Settings,
    ) -> Result<(), TlsError> {
        // SAFETY: forwarded serialization contract, no native ownership change.
        unsafe { crate::tls_post_handshake::update_shared(&self.inner, self.server, &settings)? };
        self.post_handshake_auth = settings.post_handshake_auth;
        Ok(())
    }
    /// Update settings for future SSL objects, retaining this native context.
    ///
    /// # Safety
    /// Caller must exclude every associated SSL/SSL_CTX operation.
    pub unsafe fn update_settings(
        &mut self,
        settings: crate::bindings::ssl_bio::ConnectionSettings,
    ) -> Result<(), TlsError> {
        let raw_version = |v| match v {
            TlsVersion::Tls12 => openssl_sys::TLS1_2_VERSION as u16,
            TlsVersion::Tls13 => openssl_sys::TLS1_3_VERSION as u16,
        };
        // SAFETY: forwarded serialization contract; old WIT enum is unchanged.
        unsafe {
            self.update_protocol_settings(crate::bindings::ssl_bio::ProtocolSettings {
                verify: settings.verify,
                check_hostname: settings.check_hostname,
                minimum: raw_version(settings.minimum),
                maximum: settings.maximum.map_or(0, raw_version),
            })
        }
    }
    /// Set native version bounds for future SSL objects on the same context.
    ///
    /// # Safety
    /// Caller must exclude every associated SSL/SSL_CTX operation.
    pub unsafe fn update_protocol_settings(
        &mut self,
        settings: crate::bindings::ssl_bio::ProtocolSettings,
    ) -> Result<(), TlsError> {
        if settings.check_hostname && matches!(settings.verify, VerifyMode::None) {
            return Err(invalid(
                "hostname checking requires certificate verification",
            ));
        }
        let allowed = |v| v == 0 || (0x300..=0x304).contains(&v);
        if !allowed(settings.minimum) || !allowed(settings.maximum) {
            return Err(invalid("unsupported TLS/SSL protocol version"));
        }
        let minimum = i32::from(settings.minimum);
        let maximum = i32::from(settings.maximum);
        // Validate on an unpublished builder. Inverted bounds are accepted by
        // CPython; the subsequent handshake reports no usable protocol.
        let validation = SslContextBuilder::new(SslMethod::tls()).map_err(failure)?;
        // SAFETY: unique private builder, public native setters.
        if unsafe {
            openssl_sys::SSL_CTX_set_min_proto_version(validation.as_ptr(), minimum) != 1
                || openssl_sys::SSL_CTX_set_max_proto_version(validation.as_ptr(), maximum) != 1
        } {
            return Err(invalid("unsupported TLS/SSL protocol version"));
        }
        let pointer = self.inner.as_ptr();
        // SAFETY: public native setters; caller holds the host table lock.
        unsafe {
            let old_minimum = openssl_sys::SSL_CTX_get_min_proto_version(pointer);
            let old_maximum = openssl_sys::SSL_CTX_get_max_proto_version(pointer);
            if openssl_sys::SSL_CTX_set_min_proto_version(pointer, minimum) != 1
                || openssl_sys::SSL_CTX_set_max_proto_version(pointer, maximum) != 1
            {
                let error = openssl::error::ErrorStack::get();
                openssl_sys::SSL_CTX_set_min_proto_version(pointer, old_minimum);
                openssl_sys::SSL_CTX_set_max_proto_version(pointer, old_maximum);
                return Err(failure(error));
            }
            let mut verify = verification_mode(settings.verify);
            if self.server && self.post_handshake_auth && verify.contains(SslVerifyMode::PEER) {
                verify |= SslVerifyMode::POST_HANDSHAKE;
            }
            openssl_sys::SSL_CTX_set_verify(pointer, verify.bits(), None);
        }
        self.check_hostname = settings.check_hostname;
        Ok(())
    }
    pub fn inspect_policy(&self) -> Result<crate::bindings::ssl_bio::PolicyDetails, TlsError> {
        crate::tls_policy::details(&self.inner)
    }

    /// Update policy on the same native context.
    ///
    /// # Safety
    /// Caller must exclude operations on every connection retaining this context.
    pub unsafe fn update_policy(
        &mut self,
        policy: crate::bindings::ssl_bio::ContextPolicy,
    ) -> Result<(), TlsError> {
        // SAFETY: forwarded serialization contract. No native ownership changes.
        unsafe { crate::tls_policy::apply_shared(&self.inner, &policy)? };
        self.hostname_common_name = policy.hostname_common_name;
        Ok(())
    }

    pub fn new_with_directories(
        config: Config,
        policy: crate::bindings::ssl_bio::ContextPolicy,
        settings: crate::bindings::ssl_bio::KeyExchange,
        enabled: bool,
        tls13: crate::bindings::ssl_bio::Tls13Settings,
        directories: Vec<crate::bindings::ssl_bio::TrustDirectory>,
    ) -> Result<Self, TlsError> {
        let ca = config.ca_pem.clone();
        let context = Self::new_with_tls13(config, policy, settings, enabled, tls13)?;
        Self::install_directories(context, ca, directories)
    }

    pub fn new_with_seeded_policy(
        config: Config,
        policy: crate::bindings::ssl_bio::SeededPolicy,
        settings: crate::bindings::ssl_bio::KeyExchange,
        enabled: bool,
        tls13: crate::bindings::ssl_bio::Tls13Settings,
        directories: Vec<crate::bindings::ssl_bio::TrustDirectory>,
    ) -> Result<Self, TlsError> {
        let ca = config.ca_pem.clone();
        let context = Self::build(
            config,
            Some(policy.policy),
            Some(settings),
            enabled,
            Some(tls13),
            Some(policy.seed),
            None,
        )?;
        Self::install_directories(context, ca, directories)
    }

    pub fn new_with_key_exchange_history(
        config: Config,
        policy: crate::bindings::ssl_bio::SeededPolicy,
        history: crate::bindings::ssl_bio::KeyExchangeHistory,
        enabled: bool,
        tls13: crate::bindings::ssl_bio::Tls13Settings,
        directories: Vec<crate::bindings::ssl_bio::TrustDirectory>,
    ) -> Result<Self, TlsError> {
        if history.dh_security_level > 5 {
            return Err(invalid("invalid DH load security level"));
        }
        let ca = config.ca_pem.clone();
        let context = Self::build(
            config,
            Some(policy.policy),
            Some(history.settings),
            enabled,
            Some(tls13),
            Some(policy.seed),
            Some(history.dh_security_level),
        )?;
        Self::install_directories(context, ca, directories)
    }

    pub fn new_with_history(
        config: Config,
        policy: crate::bindings::ssl_bio::SeededPolicy,
        history: crate::bindings::ssl_bio::ContextHistory,
        enabled: bool,
        tls13: crate::bindings::ssl_bio::Tls13Settings,
        directories: Vec<crate::bindings::ssl_bio::TrustDirectory>,
    ) -> Result<Self, TlsError> {
        if config.cert_pem.is_some() || config.key_pem.is_some() {
            return Err(invalid(
                "history credentials must not also be supplied in config",
            ));
        }
        if history.credentials.len() > 16
            || history.credentials.iter().any(|c| c.security_level > 5)
            || history.credentials.iter().fold(0usize, |sum, c| {
                sum.saturating_add(c.credentials.cert_pem.len())
                    .saturating_add(c.credentials.key_pem.len())
            }) > BUFFER_LIMIT
        {
            return Err(invalid("TLS credential history limit exceeded"));
        }
        let mut context = Self::new_with_key_exchange_history(
            config,
            policy,
            history.key_exchange,
            enabled,
            tls13,
            directories,
        )?;
        let current_level = context.inner.security_level();
        for entry in history.credentials {
            // SAFETY: this context is unpublished, with no SSL references.
            // Replay prior successful loads at their original security level;
            // restore the current level before any SSL can be created.
            unsafe {
                openssl_sys::SSL_CTX_set_security_level(
                    context.inner.as_ptr(),
                    entry.security_level as i32,
                )
            };
            let result = unsafe { context.update_credentials(entry.credentials) };
            unsafe {
                openssl_sys::SSL_CTX_set_security_level(
                    context.inner.as_ptr(),
                    current_level as i32,
                )
            };
            result?;
        }
        Ok(context)
    }

    fn install_directories(
        mut context: Self,
        ca: Option<Vec<u8>>,
        directories: Vec<crate::bindings::ssl_bio::TrustDirectory>,
    ) -> Result<Self, TlsError> {
        // No connection exists yet; install the lookup payload before
        // publishing this context. Connections retain its native store/lookup.
        // SAFETY: the new context/store has not escaped and no SSL references
        // exist. The exclusive builder view ends before returning the context.
        let store = unsafe {
            openssl::x509::store::X509StoreBuilderRef::from_ptr_mut(
                context.inner.cert_store().as_ptr(),
            )
        };
        let lookup = crate::tls_capath::install(store, directories)?;
        if let Some(ca) = ca {
            lookup.add_pem(context.inner.cert_store(), &ca)?;
        }
        context.directories = Some(lookup);
        Ok(context)
    }

    pub fn update_trust(
        &self,
        pem: Option<Vec<u8>>,
        default_certs: bool,
        directories: Vec<crate::bindings::ssl_bio::TrustDirectory>,
    ) -> Result<(), TlsError> {
        let lookup = self
            .directories
            .as_ref()
            .ok_or_else(|| invalid("context has no guest directory lookup"))?;
        // Validate both inputs before changing the shared native store. The
        // lookup update itself is atomic; malformed input never reaches it.
        let pem = pem.map(crate::tls_store::prepare).transpose()?;
        // update() validates and bounds retained guest anchors before its swap.
        // Cached certificates remain cached, matching native capath semantics.
        lookup.update(self.inner.cert_store(), pem.as_deref(), directories)?;
        if default_certs {
            // SAFETY: the context is retained, and the host resource-table lock
            // serializes this operation with every associated SSL operation.
            if unsafe { openssl_sys::SSL_CTX_set_default_verify_paths(self.inner.as_ptr()) } != 1 {
                return Err(failure(openssl::error::ErrorStack::get()));
            }
        }
        Ok(())
    }

    pub fn load_default_trust(
        &self,
        sources: crate::bindings::ssl_bio::DefaultTrustSources,
    ) -> Result<(), TlsError> {
        crate::tls_store::load_defaults(&self.inner, sources)
    }

    pub fn inspect_store(
        &self,
        query: crate::bindings::ssl_bio::StoreQuery,
    ) -> Result<crate::bindings::ssl_bio::StoreDetails, TlsError> {
        crate::tls_store::details(self.inner.cert_store(), query)
    }

    pub fn new(config: Config) -> Result<Self, TlsError> {
        Self::build(config, None, None, false, None, None, None)
    }

    pub fn new_with_policy(
        config: Config,
        policy: crate::bindings::ssl_bio::ContextPolicy,
    ) -> Result<Self, TlsError> {
        Self::build(config, Some(policy), None, false, None, None, None)
    }

    pub fn new_with_key_exchange(
        config: Config,
        policy: crate::bindings::ssl_bio::ContextPolicy,
        settings: crate::bindings::ssl_bio::KeyExchange,
    ) -> Result<Self, TlsError> {
        Self::build(
            config,
            Some(policy),
            Some(settings),
            false,
            None,
            None,
            None,
        )
    }

    pub fn new_with_sni(
        config: Config,
        policy: crate::bindings::ssl_bio::ContextPolicy,
        settings: crate::bindings::ssl_bio::KeyExchange,
        enabled: bool,
    ) -> Result<Self, TlsError> {
        Self::build(
            config,
            Some(policy),
            Some(settings),
            enabled,
            None,
            None,
            None,
        )
    }

    pub fn new_with_tls13(
        config: Config,
        policy: crate::bindings::ssl_bio::ContextPolicy,
        settings: crate::bindings::ssl_bio::KeyExchange,
        enabled: bool,
        tls13: crate::bindings::ssl_bio::Tls13Settings,
    ) -> Result<Self, TlsError> {
        Self::build(
            config,
            Some(policy),
            Some(settings),
            enabled,
            Some(tls13),
            None,
            None,
        )
    }

    fn build(
        config: Config,
        policy: Option<crate::bindings::ssl_bio::ContextPolicy>,
        key_exchange: Option<crate::bindings::ssl_bio::KeyExchange>,
        sni: bool,
        tls13: Option<crate::bindings::ssl_bio::Tls13Settings>,
        seed: Option<crate::bindings::ssl_bio::PolicySeed>,
        dh_security_level: Option<u32>,
    ) -> Result<Self, TlsError> {
        if [&config.ca_pem, &config.cert_pem, &config.key_pem]
            .iter()
            .map(|value| value.as_ref().map_or(0, Vec::len))
            .sum::<usize>()
            > BUFFER_LIMIT
        {
            return Err(invalid("TLS certificate configuration limit exceeded"));
        }
        if config.check_hostname && matches!(config.verify, VerifyMode::None) {
            return Err(invalid(
                "hostname checking requires client certificate verification",
            ));
        }
        if config.cert_pem.is_some() != config.key_pem.is_some() {
            return Err(invalid(
                "certificate and private key must be supplied together",
            ));
        }
        let mut builder = SslContextBuilder::new(SslMethod::tls()).map_err(failure)?;
        crate::tls_keylog::install(&mut builder)?;
        crate::tls_sni::install(&mut builder)?;
        builder
            .set_session_id_context(b"Python\0")
            .map_err(failure)?;
        builder.set_options(SslOptions::NO_COMPRESSION);
        if let Some(seed) = &seed {
            crate::tls_policy::seed(&mut builder, seed)?;
        }
        if let Some(policy) = &policy {
            crate::tls_policy::apply(&mut builder, policy)?;
        }
        if let Some(settings) = &key_exchange {
            // Restore a prior successful DH load on this unpublished builder.
            // SSL_new/handshake will still use the current effective level.
            // SAFETY: the builder exclusively owns this live native context.
            let current_level =
                unsafe { openssl_sys::SSL_CTX_get_security_level(builder.as_ptr()) } as u32;
            if settings.dh_pem.is_some() {
                if let Some(level) = dh_security_level {
                    builder.set_security_level(level);
                }
            }
            crate::tls_key_exchange::apply(&mut builder, settings)?;
            builder.set_security_level(current_level);
        }
        builder.set_mode(SslMode::ENABLE_PARTIAL_WRITE | SslMode::ACCEPT_MOVING_WRITE_BUFFER);
        builder
            .set_min_proto_version(Some(version(config.minimum)))
            .map_err(failure)?;
        builder
            .set_max_proto_version(config.maximum.map(version))
            .map_err(failure)?;
        let verify = verification_mode(config.verify);
        builder.set_verify(verify);
        if let Some(settings) = &tls13 {
            crate::tls_post_handshake::configure(&mut builder, config.server, settings)?;
        }
        if config.default_certs {
            builder.set_default_verify_paths().map_err(failure)?;
        }
        if let Some(pem) = config.ca_pem {
            crate::tls_store::add(builder.cert_store_mut(), &pem)?;
        }
        let mut credential_sizes = HashMap::new();
        if let (Some(cert_pem), Some(key_pem)) = (config.cert_pem, config.key_pem) {
            let parsed = crate::tls_credentials::parse(&crate::bindings::ssl_bio::Credentials {
                cert_pem,
                key_pem,
            })?;
            // SAFETY: the builder exclusively owns this unpublished context;
            // this borrowed reference ends before later builder mutations.
            unsafe { parsed.install(openssl::ssl::SslContextRef::from_ptr(builder.as_ptr())) }?;
            credential_sizes.insert(parsed.key_type, parsed.size);
        }
        let alpn = crate::tls_alpn::install(&mut builder, config.alpn)?;
        Ok(Self {
            inner: builder.build(),
            server: config.server,
            sni: crate::tls_sni::Policy::new(sni),
            directories: None,
            alpn,
            credential_sizes,
            post_handshake_auth: tls13.as_ref().is_some_and(|s| s.post_handshake_auth),
            check_hostname: config.check_hostname,
            hostname_common_name: policy
                .as_ref()
                .is_none_or(|policy| policy.hostname_common_name),
        })
    }

    pub fn wrap(&self, name: Option<String>) -> Result<BioConnection, TlsError> {
        self.wrap_with_role(self.server, name)
    }
    pub fn wrap_with_role(
        &self,
        server: bool,
        name: Option<String>,
    ) -> Result<BioConnection, TlsError> {
        self.wrap_with_name_requirement(server, name, true)
    }
    pub fn wrap_private_bio(
        &self,
        server: bool,
        name: Option<String>,
    ) -> Result<BioConnection, TlsError> {
        self.wrap_with_name_requirement(server, name, false)
    }
    fn wrap_with_name_requirement(
        &self,
        server: bool,
        name: Option<String>,
        require_name: bool,
    ) -> Result<BioConnection, TlsError> {
        if require_name && server && name.is_some() {
            return Err(invalid("server hostname is client-only"));
        }
        if require_name && self.check_hostname && name.is_none() {
            return Err(invalid("server hostname is required"));
        }
        let mut ssl = Ssl::new(&self.inner).map_err(failure)?;
        crate::tls_post_handshake::configure_connection(&mut ssl, server, self.post_handshake_auth);
        crate::tls_keylog::attach(&mut ssl)?;
        crate::tls_messages::attach(&mut ssl)?;
        crate::tls_sni::attach(&mut ssl, &self.sni)?;
        if require_name {
            ssl.param_mut()
                .set_hostflags(crate::tls_policy::host_flags(self.hostname_common_name));
        } else {
            // bpo43522: OpenSSL before 1.1.1l can lose hostflags in SSL_new.
            // Copy the entire native mask, including unknown bits, explicitly.
            // SAFETY: both SSL and its retained context are live and serialized;
            // getters borrow parameters without transferring ownership.
            unsafe {
                let source = openssl_sys::SSL_CTX_get0_param(self.inner.as_ptr());
                let destination = openssl_sys::SSL_get0_param(ssl.as_ptr());
                if source.is_null() || destination.is_null() {
                    return Err(failure("missing TLS verification parameters"));
                }
                openssl_sys::X509_VERIFY_PARAM_set_hostflags(
                    destination,
                    X509_VERIFY_PARAM_get_hostflags(source),
                );
            }
        }
        if let Some(name) = name {
            if name.is_empty() || name.contains('\0') {
                return Err(invalid("invalid server hostname"));
            }
            match name.parse::<IpAddr>() {
                Ok(ip) => {
                    if self.check_hostname {
                        ssl.param_mut().set_ip(ip).map_err(failure)?;
                    }
                }
                Err(_) => {
                    ssl.set_hostname(&name).map_err(failure)?;
                    if self.check_hostname {
                        ssl.param_mut().set_host(&name).map_err(failure)?;
                    }
                }
            }
        }
        if server {
            ssl.set_accept_state();
        } else {
            ssl.set_connect_state();
        }
        let transport = MemoryTransport::default();
        let eof_signal = transport.eof_signal.clone();
        let stream = SslStream::new(ssl, transport).map_err(failure)?;
        // SAFETY: fresh owned stream BIO; operations are serialized by the
        // existing connection contract. eof_signal drops after stream below.
        unsafe {
            crate::tls_transport::attach(
                openssl_sys::SSL_get_rbio(stream.ssl().as_ptr()),
                &eof_signal,
            );
        }
        Ok(BioConnection {
            stream,
            _eof_signal: eof_signal,
        })
    }
    pub fn session_stats(&self) -> crate::bindings::ssl_bio::SessionStats {
        crate::tls_session::stats(&self.inner)
    }
    pub fn retained_context(&self) -> SslContext {
        self.inner.clone()
    }
    pub fn sni_policy(&self) -> crate::tls_sni::Policy {
        self.sni.clone()
    }
}

#[derive(Default)]
struct MemoryTransport {
    input: VecDeque<u8>,
    output: VecDeque<u8>,
    eof: bool,
    eof_signal: Arc<AtomicBool>,
}
impl Read for MemoryTransport {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.input.is_empty() {
            return if self.eof {
                Ok(0)
            } else {
                Err(io::ErrorKind::WouldBlock.into())
            };
        }
        let n = buf.len().min(self.input.len());
        for byte in &mut buf[..n] {
            *byte = self.input.pop_front().unwrap();
        }
        self.eof_signal
            .store(self.eof && self.input.is_empty(), Ordering::Relaxed);
        Ok(n)
    }
}
impl Write for MemoryTransport {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let n = buf.len().min(BUFFER_LIMIT - self.output.len());
        if n == 0 {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        self.output.extend(&buf[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct BioConnection {
    stream: SslStream<MemoryTransport>,
    // Field order retains callback state even while SSL frees the stream BIO.
    _eof_signal: Arc<AtomicBool>,
}

impl Drop for BioConnection {
    fn drop(&mut self) {
        // CPython 3.12 keeps exported sessions resumable on ordinary close,
        // even when no close_notify was exchanged. This changes local cleanup
        // state only; it performs no TLS I/O or implicit shutdown handshake.
        let state = self.stream.get_shutdown();
        self.stream
            .set_shutdown(state | openssl::ssl::ShutdownState::SENT);
    }
}

impl BioConnection {
    pub fn certificate_chain(
        &self,
        verified: bool,
    ) -> Result<Option<Vec<crate::tls_certificate::BioCertificate>>, TlsError> {
        use crate::tls_certificate::BioCertificate;
        let ssl = self.stream.ssl();
        let chain = if verified {
            ssl.verified_chain()
        } else {
            ssl.peer_cert_chain()
        };
        let Some(chain) = chain else {
            return Ok(None);
        };
        if chain.len() > 256 {
            return Err(invalid("certificate chain count limit exceeded"));
        }
        let mut result = Vec::new();
        if !verified && ssl.is_server() {
            if let Some(peer) = ssl.peer_certificate() {
                result.push(BioCertificate(peer));
            }
        }
        result.extend(chain.iter().map(|cert| BioCertificate(cert.to_owned())));
        if result.len() > 256 {
            return Err(invalid("certificate chain count limit exceeded"));
        }
        let mut size = 0;
        for cert in &result {
            size += cert.public_bytes(2)?.len();
            if size > BUFFER_LIMIT {
                return Err(invalid("certificate chain DER limit exceeded"));
            }
        }
        Ok(Some(result))
    }
    pub fn keylog_enabled(&self, enabled: bool) -> Result<(), TlsError> {
        crate::tls_keylog::enable(self.stream.ssl(), enabled)
    }
    pub fn take_keylog(&self) -> Result<Vec<u8>, TlsError> {
        crate::tls_keylog::take(self.stream.ssl())
    }
    pub fn messages_enabled(&self, enabled: bool) -> Result<(), TlsError> {
        crate::tls_messages::enable(self.stream.ssl(), enabled)
    }
    pub fn take_messages(&self) -> Result<Vec<crate::bindings::ssl_bio::TlsMessage>, TlsError> {
        crate::tls_messages::take(self.stream.ssl())
    }
    fn error(&self, error: openssl::ssl::Error) -> TlsError {
        let verification = self.stream.ssl().verify_result();
        let kind = match error.code() {
            ErrorCode::WANT_READ | ErrorCode::WANT_CLIENT_HELLO_CB => ErrorKind::WantRead,
            ErrorCode::WANT_WRITE => ErrorKind::WantWrite,
            ErrorCode::ZERO_RETURN => ErrorKind::ZeroReturn,
            _ if verification.as_raw() != 0
                && error.ssl_error().is_some_and(|stack| {
                    stack
                        .errors()
                        .iter()
                        .any(|e| e.reason() == Some("certificate verify failed"))
                }) =>
            {
                ErrorKind::Verification
            }
            ErrorCode::SYSCALL if self.stream.get_ref().eof => ErrorKind::Eof,
            _ if error.ssl_error().is_some_and(|stack| {
                stack
                    .errors()
                    .iter()
                    .any(|e| e.reason() == Some("unexpected eof while reading"))
            }) =>
            {
                ErrorKind::Eof
            }
            _ => ErrorKind::Failure,
        };
        TlsError {
            kind,
            message: error.to_string(),
            verify_code: (verification.as_raw() != 0).then_some(verification.as_raw()),
        }
    }
    pub fn feed(&mut self, bytes: Vec<u8>) -> Result<(), TlsError> {
        let io = self.stream.get_mut();
        if io.eof {
            return Err(invalid("cannot write after EOF"));
        }
        if bytes.len() > BUFFER_LIMIT - io.input.len() {
            return Err(invalid("TLS input buffer limit exceeded"));
        }
        io.input.extend(bytes);
        Ok(())
    }
    pub fn feed_eof(&mut self) {
        let transport = self.stream.get_mut();
        transport.eof = true;
        transport
            .eof_signal
            .store(transport.input.is_empty(), Ordering::Relaxed);
    }
    pub fn drain(&mut self, size: u32) -> Vec<u8> {
        let io = self.stream.get_mut();
        let count = (size as usize).min(io.output.len());
        io.output.drain(..count).collect()
    }
    pub fn handshake(&mut self) -> Result<(), TlsError> {
        self.stream.do_handshake().map_err(|e| self.error(e))
    }
    pub fn read(&mut self, size: u32) -> Result<Vec<u8>, TlsError> {
        if size as usize > BUFFER_LIMIT {
            return Err(invalid("TLS read limit exceeded"));
        }
        if size == 0 {
            return Ok(Vec::new());
        }
        let mut buf = vec![0; size as usize];
        let n = self.stream.ssl_read(&mut buf).map_err(|e| self.error(e))?;
        buf.truncate(n);
        Ok(buf)
    }
    pub fn write(&mut self, bytes: &[u8]) -> Result<u32, TlsError> {
        if bytes.len() > BUFFER_LIMIT {
            return Err(invalid("TLS write limit exceeded"));
        }
        if bytes.is_empty() {
            return self.write_empty();
        }
        self.stream
            .ssl_write(bytes)
            .map(|n| n as u32)
            .map_err(|e| self.error(e))
    }
    fn write_empty(&mut self) -> Result<u32, TlsError> {
        // openssl-rs skips empty writes. CPython calls SSL_write_ex even for
        // zero bytes: implicit handshake, retry and shutdown rules still apply.
        let ssl = self.stream.ssl().as_ptr();
        let mut written = 0;
        // SAFETY: this resource is exclusively borrowed on the calling thread.
        // The zero-length input pointer and output counter remain valid for the
        // call. Classify failures immediately, without intervening OpenSSL calls
        // and with an initially empty thread-local error queue.
        let (result, code) = unsafe {
            openssl_sys::ERR_clear_error();
            let result = openssl_sys::SSL_write_ex(ssl, b"".as_ptr().cast(), 0, &mut written);
            let code = if result > 0 {
                openssl_sys::SSL_ERROR_NONE
            } else {
                openssl_sys::SSL_get_error(ssl, result)
            };
            (result, code)
        };
        if result > 0 {
            debug_assert_eq!(written, 0);
            return Ok(0);
        }
        let kind = match code {
            openssl_sys::SSL_ERROR_WANT_READ | openssl_sys::SSL_ERROR_WANT_CLIENT_HELLO_CB => {
                ErrorKind::WantRead
            }
            openssl_sys::SSL_ERROR_WANT_WRITE => ErrorKind::WantWrite,
            openssl_sys::SSL_ERROR_ZERO_RETURN => ErrorKind::ZeroReturn,
            _ => {
                let stack = openssl::error::ErrorStack::get();
                if !stack.errors().is_empty() {
                    return Err(self.error(stack.into()));
                }
                if code == openssl_sys::SSL_ERROR_SYSCALL && self.stream.get_ref().eof {
                    ErrorKind::Eof
                } else {
                    ErrorKind::Failure
                }
            }
        };
        Err(TlsError {
            kind,
            message: format!("TLS empty write failed (SSL error {code})"),
            verify_code: None,
        })
    }
    pub fn shutdown(&mut self) -> Result<(), TlsError> {
        // Like CPython, try receiving after the first successful send. The
        // peer's close_notify may already be buffered without an SSL_read.
        // Two attempts bound this operation; WANT_READ/WRITE still return.
        for _ in 0..2 {
            if self.stream.shutdown().map_err(|e| self.error(e))?
                == openssl::ssl::ShutdownResult::Received
            {
                return Ok(());
            }
        }
        Err(TlsError {
            kind: ErrorKind::WantRead,
            message: "waiting for peer close_notify".into(),
            verify_code: None,
        })
    }
    pub fn info(&self) -> Result<ConnectionInfo, TlsError> {
        let ssl = self.stream.ssl();
        Ok(ConnectionInfo {
            version: ssl.is_init_finished().then(|| ssl.version_str().to_owned()),
            cipher: ssl.current_cipher().map(|c| c.name().to_owned()),
            cipher_bits: ssl.current_cipher().map_or(0, |c| c.bits().secret as u32),
            alpn: ssl
                .selected_alpn_protocol()
                .map(|p| String::from_utf8_lossy(p).into_owned()),
            peer_cert_der: ssl
                .peer_certificate()
                .map(|c| c.to_der().map_err(failure))
                .transpose()?,
            pending: ssl.pending() as u32,
            input_pending: self.stream.get_ref().input.len() as u32,
        })
    }
    pub fn selected_alpn_bytes(&self) -> Result<Option<Vec<u8>>, TlsError> {
        self.stream
            .ssl()
            .selected_alpn_protocol()
            .map(|bytes| {
                if bytes.len() > 255 {
                    return Err(failure("negotiated ALPN name limit exceeded"));
                }
                Ok(bytes.to_vec())
            })
            .transpose()
    }
}

impl BioConnection {
    pub fn cipher_info(&self) -> Result<Option<crate::bindings::ssl_bio::CipherDetails>, TlsError> {
        self.stream
            .ssl()
            .current_cipher()
            .map(crate::tls_policy::cipher_details)
            .transpose()
    }
    pub fn channel_binding(&self) -> Result<Option<Vec<u8>>, TlsError> {
        crate::tls_connection::channel_binding(self.stream.ssl())
    }

    pub fn compression(&self) -> Result<Option<String>, TlsError> {
        crate::tls_connection::compression(self.stream.ssl())
    }
    pub fn session(&self) -> Option<crate::tls_session::BioSession> {
        crate::tls_session::BioSession::capture(self.stream.ssl())
    }
    pub fn set_session(
        &mut self,
        session: &crate::tls_session::BioSession,
    ) -> Result<(), TlsError> {
        use foreign_types::ForeignTypeRef;
        // SAFETY: this exclusive connection borrow owns the SSL through its
        // SslStream. No immutable SSL borrow survives the pointer extraction.
        let ssl = unsafe { openssl::ssl::SslRef::from_ptr_mut(self.stream.ssl().as_ptr()) };
        session.apply(ssl)
    }
    pub fn session_reused(&self) -> bool {
        self.stream.ssl().session_reused()
    }
    pub fn verify_client_post_handshake(&mut self) -> Result<(), TlsError> {
        use foreign_types::ForeignTypeRef;
        // SAFETY: exclusive connection owns SSL; no shared SSL borrow survives
        // extraction. The native request operation never calls guest code.
        let ssl = unsafe { openssl::ssl::SslRef::from_ptr_mut(self.stream.ssl().as_ptr()) };
        crate::tls_post_handshake::request(ssl)
    }
    pub fn pending_sni(&self) -> Result<Option<crate::bindings::ssl_bio::SniRequest>, TlsError> {
        crate::tls_sni::pending(self.stream.ssl())
    }
    pub fn resolve_sni(&self, alert: Option<u8>) -> Result<(), TlsError> {
        crate::tls_sni::resolve(self.stream.ssl(), alert)
    }
    pub fn set_context(
        &mut self,
        context: &SslContext,
        policy: &crate::tls_sni::Policy,
    ) -> Result<(), TlsError> {
        use foreign_types::ForeignTypeRef;
        // SAFETY: exclusive BioConnection owns the SSL, with no live shared
        // SSL borrow after pointer extraction. The retained context is owned.
        let ssl = unsafe { openssl::ssl::SslRef::from_ptr_mut(self.stream.ssl().as_ptr()) };
        crate::tls_sni::change_context(ssl, context, policy)
    }
    pub fn shared_ciphers(
        &self,
    ) -> Result<Option<Vec<crate::bindings::ssl_bio::SharedCipher>>, TlsError> {
        crate::tls_connection::shared_ciphers(self.stream.ssl())
    }
    pub fn peer_certificate(&self) -> Result<Option<PeerCertificate>, TlsError> {
        let ssl = self.stream.ssl();
        if !ssl.is_init_finished() {
            return Err(invalid("handshake not done yet"));
        }
        let Some(cert) = ssl.peer_certificate() else {
            return Ok(None);
        };
        // CPython 3.12 reads the current parent SSL_CTX mode here. This
        // controls decoded-vs-empty output, not the SSL's actual validation.
        if !ssl
            .ssl_context()
            .verify_mode()
            .contains(SslVerifyMode::PEER)
        {
            return Ok(Some(PeerCertificate::Unverified));
        }
        crate::tls_certificate::decode(&cert).map(|value| Some(PeerCertificate::Verified(value)))
    }
}

fn verification_mode(mode: VerifyMode) -> SslVerifyMode {
    match mode {
        VerifyMode::None => SslVerifyMode::NONE,
        VerifyMode::Optional => SslVerifyMode::PEER,
        VerifyMode::Required => SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openssl::{
        asn1::Asn1Time,
        hash::MessageDigest,
        pkey::PKey,
        rsa::Rsa,
        x509::{
            extension::{BasicConstraints, SubjectAlternativeName},
            X509NameBuilder, X509,
        },
    };

    fn certificate() -> (Vec<u8>, Vec<u8>) {
        let key = PKey::from_rsa(Rsa::generate(2048).unwrap()).unwrap();
        let mut name = X509NameBuilder::new().unwrap();
        name.append_entry_by_text("CN", "localhost").unwrap();
        let name = name.build();
        let mut cert = X509::builder().unwrap();
        cert.set_version(2).unwrap();
        cert.set_subject_name(&name).unwrap();
        cert.set_issuer_name(&name).unwrap();
        cert.set_pubkey(&key).unwrap();
        cert.set_not_before(&Asn1Time::days_from_now(0).unwrap())
            .unwrap();
        cert.set_not_after(&Asn1Time::days_from_now(1).unwrap())
            .unwrap();
        cert.append_extension(BasicConstraints::new().critical().ca().build().unwrap())
            .unwrap();
        let san = SubjectAlternativeName::new()
            .dns("localhost")
            .ip("127.0.0.1")
            .build(&cert.x509v3_context(None, None))
            .unwrap();
        cert.append_extension(san).unwrap();
        cert.sign(&key, MessageDigest::sha256()).unwrap();
        (
            cert.build().to_pem().unwrap(),
            key.private_key_to_pem_pkcs8().unwrap(),
        )
    }
    fn config(server: bool) -> Config {
        Config {
            server,
            verify: if server {
                VerifyMode::None
            } else {
                VerifyMode::Required
            },
            check_hostname: !server,
            default_certs: false,
            ca_pem: None,
            cert_pem: None,
            key_pem: None,
            alpn: vec!["h2".into(), "http/1.1".into()],
            minimum: TlsVersion::Tls12,
            maximum: None,
        }
    }
    #[test]
    fn settings_updates_preserve_context_and_copy_ssl_verification() {
        use crate::bindings::ssl_bio::{
            ConnectionSettings, ContextPolicy, KeyExchange, Tls13Settings,
        };
        let mut initial = config(true);
        initial.check_hostname = false;
        let mut context = BioContext::new_with_tls13(
            initial,
            ContextPolicy {
                options: 0,
                verify_flags: 0,
                ciphers: None,
                hostname_common_name: true,
            },
            KeyExchange {
                dh_pem: None,
                curve: None,
            },
            false,
            Tls13Settings {
                post_handshake_auth: true,
                num_tickets: 2,
            },
        )
        .unwrap();
        let before = context.wrap(None).unwrap();
        let pointer = context.inner.as_ptr();
        // SAFETY: all objects remain local with no concurrent native calls.
        unsafe {
            context.update_settings(ConnectionSettings {
                verify: VerifyMode::Optional,
                check_hostname: false,
                minimum: TlsVersion::Tls13,
                maximum: Some(TlsVersion::Tls12),
            })
        }
        .unwrap();
        let after = context.wrap(None).unwrap();
        assert_eq!(context.inner.as_ptr(), pointer);
        assert_eq!(before.stream.ssl().verify_mode(), SslVerifyMode::NONE);
        assert_eq!(
            after.stream.ssl().verify_mode(),
            SslVerifyMode::PEER | SslVerifyMode::POST_HANDSHAKE
        );
        assert_eq!(
            after.stream.ssl().ssl_context().as_ptr(),
            before.stream.ssl().ssl_context().as_ptr()
        );
        assert!(unsafe {
            context.update_settings(ConnectionSettings {
                verify: VerifyMode::None,
                check_hostname: true,
                minimum: TlsVersion::Tls12,
                maximum: None,
            })
        }
        .is_err());
        assert!(!context.check_hostname);
        assert_eq!(
            context.inner.verify_mode(),
            SslVerifyMode::PEER | SslVerifyMode::POST_HANDSHAKE
        );
    }
    #[test]
    fn protocol_updates_copy_bounds_and_reject_invalid_input_atomically() {
        use crate::bindings::ssl_bio::ProtocolSettings;
        let mut context = BioContext::new(config(true)).unwrap();
        let before = context.wrap(None).unwrap();
        let pointer = context.inner.as_ptr();
        let settings = |minimum, maximum| ProtocolSettings {
            verify: VerifyMode::None,
            check_hostname: false,
            minimum,
            maximum,
        };
        // SAFETY: local context and connections, no concurrent native calls.
        unsafe { context.update_protocol_settings(settings(0x301, 0x302)) }.unwrap();
        let after = context.wrap(None).unwrap();
        assert_eq!(context.inner.as_ptr(), pointer);
        assert_eq!(
            unsafe { openssl_sys::SSL_get_min_proto_version(before.stream.ssl().as_ptr()) },
            0x303
        );
        assert_eq!(
            unsafe { openssl_sys::SSL_get_min_proto_version(after.stream.ssl().as_ptr()) },
            0x301
        );
        assert_eq!(
            unsafe { openssl_sys::SSL_get_max_proto_version(after.stream.ssl().as_ptr()) },
            0x302
        );
        assert!(unsafe { context.update_protocol_settings(settings(0x304, 0xffff)) }.is_err());
        assert_eq!(
            unsafe { openssl_sys::SSL_CTX_get_min_proto_version(pointer) },
            0x301
        );
        assert_eq!(
            unsafe { openssl_sys::SSL_CTX_get_max_proto_version(pointer) },
            0x302
        );
        unsafe { context.update_protocol_settings(settings(0, 0)) }.unwrap();
        assert_eq!(
            unsafe { openssl_sys::SSL_CTX_get_min_proto_version(pointer) },
            0
        );
        assert_eq!(
            unsafe { openssl_sys::SSL_CTX_get_max_proto_version(pointer) },
            0
        );
        assert_eq!(context.inner.security_level(), 2);
        let versions = crate::tls_protocols::capabilities();
        assert!(versions.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(versions
            .iter()
            .all(|version| (0x300..=0x304).contains(version)));
    }
    fn pair(trusted: bool, name: &str) -> (BioConnection, BioConnection) {
        pair_with_verify(trusted, name, VerifyMode::Required)
    }
    #[test]
    fn one_context_wraps_both_roles_with_role_specific_pha() {
        use crate::bindings::ssl_bio::Tls13Settings;
        let (cert, key) = certificate();
        let mut settings = config(false);
        settings.check_hostname = false;
        settings.ca_pem = Some(cert.clone());
        settings.cert_pem = Some(cert);
        settings.key_pem = Some(key);
        let mut context = BioContext::new(settings).unwrap();
        // SAFETY: all native objects remain local, no concurrent operations.
        unsafe {
            context.update_tls13(Tls13Settings {
                post_handshake_auth: true,
                num_tickets: 2,
            })
        }
        .unwrap();
        let client = context
            .wrap_with_role(false, Some("localhost".into()))
            .unwrap();
        let server = context.wrap_with_role(true, None).unwrap();
        assert!(!client.stream.ssl().is_server());
        assert!(server.stream.ssl().is_server());
        assert_eq!(
            client.stream.ssl().ssl_context().as_ptr(),
            server.stream.ssl().ssl_context().as_ptr()
        );
        assert!(!client
            .stream
            .ssl()
            .verify_mode()
            .contains(SslVerifyMode::POST_HANDSHAKE));
        assert!(server
            .stream
            .ssl()
            .verify_mode()
            .contains(SslVerifyMode::POST_HANDSHAKE));
        assert!(context
            .wrap_with_role(true, Some("localhost".into()))
            .is_err());
        drop(client);
        drop(server);
        unsafe {
            context.update_tls13(Tls13Settings {
                post_handshake_auth: false,
                num_tickets: 2,
            })
        }
        .unwrap();
        let mut client = context
            .wrap_with_role(false, Some("localhost".into()))
            .unwrap();
        let mut server = context.wrap_with_role(true, None).unwrap();
        drop(context);
        handshake(&mut client, &mut server).unwrap();
        assert!(server.peer_certificate().unwrap().is_some());
    }

    #[test]
    fn live_sni_policy_outlives_context_resource() {
        for enabled in [false, true] {
            let (cert, key) = certificate();
            let mut cc = config(false);
            cc.ca_pem = Some(cert.clone());
            let mut sc = config(true);
            sc.cert_pem = Some(cert);
            sc.key_pem = Some(key);
            let context = BioContext::new(sc).unwrap();
            context.update_sni(!enabled);
            let mut client = BioContext::new(cc)
                .unwrap()
                .wrap(Some("localhost".into()))
                .unwrap();
            let mut server = context.wrap(None).unwrap();
            context.update_sni(enabled);
            let policy = context.sni_policy();
            drop(context);
            if enabled {
                assert_eq!(client.handshake().unwrap_err().kind, ErrorKind::WantRead);
                transfer(&mut client, &mut server);
                assert_eq!(server.handshake().unwrap_err().kind, ErrorKind::WantRead);
                assert_eq!(
                    server
                        .pending_sni()
                        .unwrap()
                        .unwrap()
                        .server_name
                        .as_deref(),
                    Some("localhost")
                );
                // Disabling during the suspended callback cannot lose its
                // pending result. The SSL retains both context policies.
                policy.update(false);
                server.resolve_sni(None).unwrap();
            }
            handshake(&mut client, &mut server).unwrap();
            assert!(server.pending_sni().unwrap().is_none());
        }
    }

    #[test]
    fn live_alpn_outlives_context_resources_and_copies_client_defaults() {
        let (cert, key) = certificate();
        let mut client_config = config(false);
        client_config.ca_pem = Some(cert.clone());
        let mut server_config = config(true);
        server_config.cert_pem = Some(cert);
        server_config.key_pem = Some(key);
        let client_context = BioContext::new(client_config).unwrap();
        let server_context = BioContext::new(server_config).unwrap();
        let mut client = client_context.wrap(Some("localhost".into())).unwrap();
        let mut server = server_context.wrap(None).unwrap();
        // SAFETY: local resources, no associated SSL operations run concurrently.
        unsafe {
            client_context.update_alpn(vec!["other".into()]).unwrap();
            server_context.update_alpn(vec!["http/1.1".into()]).unwrap();
        }
        assert!(unsafe { server_context.update_alpn(vec![String::new()]) }.is_err());
        drop(client_context);
        drop(server_context);
        handshake(&mut client, &mut server).unwrap();
        assert_eq!(client.info().unwrap().alpn.as_deref(), Some("http/1.1"));
        assert_eq!(server.info().unwrap().alpn.as_deref(), Some("http/1.1"));
    }

    #[test]
    fn live_credentials_copy_slots_and_outlive_context_and_input_owners() {
        use crate::bindings::ssl_bio::Credentials;
        let original = include_bytes!("../../../../tests/fixtures/ssl/server.crt").to_vec();
        let replacement = include_bytes!("../../../../tests/fixtures/ssl/leaf.crt").to_vec();
        let key = include_bytes!("../../../../tests/fixtures/ssl/server.key").to_vec();
        let mut settings = config(true);
        settings.cert_pem = Some(original.clone());
        settings.key_pem = Some(key.clone());
        let mut context = BioContext::new(settings).unwrap();
        let before = context.wrap(None).unwrap();
        let pointer = context.inner.as_ptr();
        // SAFETY: local resources with no concurrent native calls.
        unsafe {
            context.update_credentials(Credentials {
                cert_pem: replacement.clone(),
                key_pem: key.clone(),
            })
        }
        .unwrap();
        assert_eq!(context.inner.as_ptr(), pointer);
        let mut after = context.wrap(None).unwrap();
        // Failure must preserve the last complete pair, including budget errors.
        let mut padded = replacement.clone();
        padded.resize(BUFFER_LIMIT, b'\n');
        assert!(unsafe {
            context.update_credentials(Credentials {
                cert_pem: padded,
                key_pem: key.clone(),
            })
        }
        .is_err());
        assert!(unsafe {
            context.update_credentials(Credentials {
                cert_pem: replacement.clone(),
                key_pem: include_bytes!("../../../../tests/fixtures/ssl/ec.key").to_vec(),
            })
        }
        .is_err());
        assert!(unsafe {
            context.update_credentials(Credentials {
                cert_pem: include_bytes!("../../../../tests/fixtures/ssl/weak-rsa.crt").to_vec(),
                key_pem: include_bytes!("../../../../tests/fixtures/ssl/weak-rsa.key").to_vec(),
            })
        }
        .is_err());
        assert_eq!(context.credential_sizes.len(), 1);
        let final_object = context.wrap(None).unwrap();
        drop(context);
        let der =
            |object: &BioConnection| object.stream.ssl().certificate().unwrap().to_der().unwrap();
        assert_eq!(
            der(&before),
            X509::from_pem(&original).unwrap().to_der().unwrap()
        );
        assert_eq!(
            der(&after),
            X509::from_pem(&replacement).unwrap().to_der().unwrap()
        );
        assert_eq!(der(&final_object), der(&after));
        let mut client_settings = config(false);
        client_settings.ca_pem =
            Some(include_bytes!("../../../../tests/fixtures/ssl/ca.crt").to_vec());
        let mut client = BioContext::new(client_settings)
            .unwrap()
            .wrap(Some("localhost".into()))
            .unwrap();
        handshake(&mut client, &mut after).unwrap();
        assert_eq!(
            client
                .stream
                .ssl()
                .peer_certificate()
                .unwrap()
                .to_der()
                .unwrap(),
            der(&after)
        );
    }

    #[test]
    fn updated_dh_parameters_outlive_context_resource_and_rust_owner() {
        use crate::bindings::ssl_bio::{ContextPolicy, KeyExchangeUpdate};
        let cert = include_bytes!("../../../../tests/fixtures/ssl/server.crt").to_vec();
        let key = include_bytes!("../../../../tests/fixtures/ssl/server.key").to_vec();
        let mut client_config = config(false);
        client_config.ca_pem = Some(cert.clone());
        client_config.maximum = Some(TlsVersion::Tls12);
        let mut server_config = config(true);
        server_config.cert_pem = Some(cert);
        server_config.key_pem = Some(key);
        server_config.maximum = Some(TlsVersion::Tls12);
        let policy = ContextPolicy {
            options: 0,
            verify_flags: 0,
            ciphers: Some("DHE-RSA-AES256-GCM-SHA384".into()),
            hostname_common_name: true,
        };
        let client_context = BioContext::new_with_policy(client_config, policy.clone()).unwrap();
        let server_context = BioContext::new_with_policy(server_config, policy).unwrap();
        // SAFETY: all resources remain local; no native calls run concurrently.
        unsafe {
            server_context.update_key_exchange(KeyExchangeUpdate::DhPem(
                include_bytes!("../../../../tests/fixtures/ssl/dh2048.pem").to_vec(),
            ))
        }
        .unwrap();
        assert!(unsafe {
            server_context.update_key_exchange(KeyExchangeUpdate::DhPem(
                include_bytes!("../../../../tests/fixtures/ssl/dh1024.pem").to_vec(),
            ))
        }
        .is_err());
        let mut client = client_context.wrap(Some("localhost".into())).unwrap();
        let mut server = server_context.wrap(None).unwrap();
        drop(client_context);
        drop(server_context);
        handshake(&mut client, &mut server).unwrap();
        assert_eq!(
            client.info().unwrap().cipher.as_deref(),
            Some("DHE-RSA-AES256-GCM-SHA384")
        );
    }

    fn pair_with_verify(
        trusted: bool,
        name: &str,
        verify: VerifyMode,
    ) -> (BioConnection, BioConnection) {
        let (cert, key) = certificate();
        let mut client_config = config(false);
        client_config.check_hostname = !matches!(verify, VerifyMode::None);
        client_config.verify = verify;
        if trusted {
            client_config.ca_pem = Some(cert.clone());
        }
        let mut server_config = config(true);
        server_config.cert_pem = Some(cert);
        server_config.key_pem = Some(key);
        let client = BioContext::new(client_config)
            .unwrap()
            .wrap(Some(name.into()))
            .unwrap();
        let server = BioContext::new(server_config).unwrap().wrap(None).unwrap();
        // Context handles can be dropped while connections remain live.
        (client, server)
    }
    fn transfer(source: &mut BioConnection, destination: &mut BioConnection) {
        let bytes = source.drain(BUFFER_LIMIT as u32);
        for chunk in bytes.chunks(7) {
            destination.feed(chunk.to_vec()).unwrap();
        }
    }
    fn handshake(client: &mut BioConnection, server: &mut BioConnection) -> Result<(), TlsError> {
        for _ in 0..32 {
            let a = client.handshake();
            if let Err(e) = &a {
                if !matches!(e.kind, ErrorKind::WantRead | ErrorKind::WantWrite) {
                    return a;
                }
            }
            transfer(client, server);
            let b = server.handshake();
            if let Err(e) = &b {
                if !matches!(e.kind, ErrorKind::WantRead | ErrorKind::WantWrite) {
                    return b;
                }
            }
            transfer(server, client);
            if a.is_ok() && b.is_ok() {
                return Ok(());
            }
        }
        panic!("TLS handshake did not complete");
    }
    #[test]
    fn empty_writes_drive_handshake_and_obey_shutdown() {
        for protocol in [TlsVersion::Tls12, TlsVersion::Tls13] {
            let (cert, key) = certificate();
            let mut cc = config(false);
            cc.ca_pem = Some(cert.clone());
            cc.minimum = protocol;
            cc.maximum = Some(protocol);
            let mut sc = config(true);
            sc.cert_pem = Some(cert);
            sc.key_pem = Some(key);
            sc.minimum = protocol;
            sc.maximum = Some(protocol);
            let mut client = BioContext::new(cc)
                .unwrap()
                .wrap(Some("localhost".into()))
                .unwrap();
            let mut server = BioContext::new(sc).unwrap().wrap(None).unwrap();
            assert_eq!(client.write(b"").unwrap_err().kind, ErrorKind::WantRead);
            assert!(!client.stream.get_ref().output.is_empty());
            for _ in 0..32 {
                transfer(&mut client, &mut server);
                let a = server.write(b"");
                transfer(&mut server, &mut client);
                let b = client.write(b"");
                for result in [&a, &b] {
                    if let Err(error) = result {
                        assert!(matches!(
                            error.kind,
                            ErrorKind::WantRead | ErrorKind::WantWrite
                        ));
                    }
                }
                if a.is_ok() && b.is_ok() {
                    break;
                }
            }
            assert!(client.stream.ssl().is_init_finished());
            assert!(server.stream.ssl().is_init_finished());
            transfer(&mut client, &mut server);
            assert_eq!(client.write(b"").unwrap(), 0);
            assert!(client.stream.get_ref().output.is_empty());
            client.write(b"after empty writes").unwrap();
            transfer(&mut client, &mut server);
            assert_eq!(server.read(100).unwrap(), b"after empty writes");
            assert_eq!(client.shutdown().unwrap_err().kind, ErrorKind::WantRead);
            let error = client.write(b"").unwrap_err();
            assert_eq!(error.kind, ErrorKind::Failure);
            assert!(error.message.contains("protocol is shutdown"), "{error:?}");
        }
    }
    #[test]
    fn empty_write_handles_backpressure_and_bad_write_retry() {
        let (mut client, mut server) = pair(true, "localhost");
        client.stream.get_mut().output.resize(BUFFER_LIMIT, 0);
        let stale = openssl::x509::X509::from_der(b"invalid certificate").unwrap_err();
        stale.errors().last().unwrap().put();
        assert_eq!(client.write(b"").unwrap_err().kind, ErrorKind::WantWrite);
        assert!(openssl::error::ErrorStack::get().errors().is_empty());
        client.drain(BUFFER_LIMIT as u32);
        assert_eq!(client.write(b"").unwrap_err().kind, ErrorKind::WantRead);
        handshake(&mut client, &mut server).unwrap();
        client.stream.get_mut().output.resize(BUFFER_LIMIT, 0);
        assert_eq!(
            client.write(b"pending").unwrap_err().kind,
            ErrorKind::WantWrite
        );
        let error = client.write(b"").unwrap_err();
        assert_eq!(error.kind, ErrorKind::Failure);
        // OpenSSL versions reject the changed retry length with either reason.
        assert!(
            error.message.contains("bad write retry") || error.message.contains("bad length"),
            "{error:?}"
        );
    }
    #[test]
    fn empty_write_preserves_certificate_verification_error() {
        let (mut client, mut server) = pair(false, "localhost");
        assert_eq!(client.write(b"").unwrap_err().kind, ErrorKind::WantRead);
        transfer(&mut client, &mut server);
        assert_eq!(server.write(b"").unwrap_err().kind, ErrorKind::WantRead);
        transfer(&mut server, &mut client);
        let error = client.write(b"").unwrap_err();
        assert_eq!(error.kind, ErrorKind::Verification);
        assert!(error.verify_code.is_some());
        assert!(error.message.contains("certificate verify failed"));
    }
    #[test]
    fn keylog_is_opt_in_and_retained_with_connections() {
        let (mut client, mut server) = pair(true, "localhost");
        client.keylog_enabled(true).unwrap();
        server.keylog_enabled(true).unwrap();
        handshake(&mut client, &mut server).unwrap();
        for peer in [&client, &server] {
            let lines = peer.take_keylog().unwrap();
            assert!(lines
                .split(|byte| *byte == b'\n')
                .any(|line| line.starts_with(b"CLIENT_HANDSHAKE_TRAFFIC_SECRET ")));
            assert!(peer.take_keylog().unwrap().is_empty());
            peer.keylog_enabled(false).unwrap();
        }
        let (mut client, mut server) = pair(true, "localhost");
        handshake(&mut client, &mut server).unwrap();
        assert!(client.take_keylog().unwrap().is_empty());
        assert!(server.take_keylog().unwrap().is_empty());
    }
    #[test]
    fn protocol_messages_are_opt_in_owned_and_exclude_application_plaintext() {
        let (mut client, mut server) = pair(true, "localhost");
        client.messages_enabled(true).unwrap();
        server.messages_enabled(true).unwrap();
        handshake(&mut client, &mut server).unwrap();
        for peer in [&client, &server] {
            let messages = peer.take_messages().unwrap();
            assert!(messages
                .iter()
                .any(|m| m.content_type == 22 && m.data.first() == Some(&1)));
            assert!(messages
                .iter()
                .any(|m| m.content_type == 22 && m.data.first() == Some(&20)));
            assert!(messages.iter().all(|m| m.content_type != 23));
            assert!(peer.take_messages().unwrap().is_empty());
        }
        client.write(b"application secret").unwrap();
        transfer(&mut client, &mut server);
        assert_eq!(server.read(100).unwrap(), b"application secret");
        for peer in [&client, &server] {
            let messages = peer.take_messages().unwrap();
            assert!(messages
                .iter()
                .all(|m| !m.data.windows(18).any(|w| w == b"application secret")));
            peer.messages_enabled(false).unwrap();
        }
        client.write(b"disabled").unwrap();
        assert!(client.take_messages().unwrap().is_empty());
        let (mut client, mut server) = pair(true, "localhost");
        handshake(&mut client, &mut server).unwrap();
        assert!(client.take_messages().unwrap().is_empty());
        assert!(server.take_messages().unwrap().is_empty());
    }
    #[test]
    fn trusted_tls_exchange_and_alpn() {
        let (mut client, mut server) = pair(true, "localhost");
        assert_eq!(client.handshake().unwrap_err().kind, ErrorKind::WantRead);
        transfer(&mut client, &mut server);
        handshake(&mut client, &mut server).unwrap();
        let info = client.info().unwrap();
        assert_eq!(info.alpn.as_deref(), Some("h2"));
        assert!(info.peer_cert_der.is_some());
        assert!(info.cipher_bits >= 128);
        assert_eq!(client.write(b"request").unwrap(), 7);
        transfer(&mut client, &mut server);
        assert_eq!(server.read(1024).unwrap(), b"request");
        server.write(b"response").unwrap();
        transfer(&mut server, &mut client);
        assert_eq!(client.read(1024).unwrap(), b"response");
    }
    #[test]
    fn suspended_sni_retains_replacement_and_delivers_alert() {
        use crate::bindings::ssl_bio::{ContextPolicy, KeyExchange};
        for alert in [None, Some(112)] {
            let (cert, key) = certificate();
            let mut cc = config(false);
            cc.ca_pem = Some(cert.clone());
            let mut sc = config(true);
            sc.cert_pem = Some(cert);
            sc.key_pem = Some(key);
            let replacement = BioContext::new(sc.clone()).unwrap();
            let original = BioContext::new_with_sni(
                sc,
                ContextPolicy {
                    options: 0,
                    verify_flags: 0,
                    ciphers: None,
                    hostname_common_name: true,
                },
                KeyExchange {
                    dh_pem: None,
                    curve: None,
                },
                true,
            )
            .unwrap();
            let mut client = BioContext::new(cc)
                .unwrap()
                .wrap(Some("localhost".into()))
                .unwrap();
            let mut server = original.wrap(None).unwrap();
            drop(original);
            assert!(client.resolve_sni(None).is_err());
            assert_eq!(client.handshake().unwrap_err().kind, ErrorKind::WantRead);
            transfer(&mut client, &mut server);
            assert_eq!(server.handshake().unwrap_err().kind, ErrorKind::WantRead);
            assert_eq!(
                server
                    .pending_sni()
                    .unwrap()
                    .unwrap()
                    .server_name
                    .as_deref(),
                Some("localhost")
            );
            server
                .set_context(&replacement.retained_context(), &replacement.sni_policy())
                .unwrap();
            drop(replacement);
            server.resolve_sni(alert).unwrap();
            assert!(server.resolve_sni(None).is_err());
            if alert.is_some() {
                assert_eq!(server.handshake().unwrap_err().kind, ErrorKind::Failure);
                transfer(&mut server, &mut client);
                let error = client.handshake().unwrap_err();
                assert_eq!(error.kind, ErrorKind::Failure);
                assert!(error.message.contains("unrecognized name"));
            } else {
                handshake(&mut client, &mut server).unwrap();
                assert!(server.pending_sni().unwrap().is_none());
                assert_eq!(
                    client.channel_binding().unwrap(),
                    server.channel_binding().unwrap()
                );
                client.write(b"SNI").unwrap();
                transfer(&mut client, &mut server);
                assert_eq!(server.read(16).unwrap(), b"SNI");
            }
        }
    }
    #[test]
    fn live_trust_updates_retain_context_and_outlive_its_resource() {
        use crate::bindings::ssl_bio::{
            ContextPolicy, KeyExchange, StoreQuery, Tls13Settings, TrustDirectory, TrustFile,
        };
        for directory in [false, true] {
            let (cert, key) = certificate();
            let mut sc = config(true);
            sc.cert_pem = Some(cert.clone());
            sc.key_pem = Some(key);
            let server = BioContext::new(sc).unwrap();
            let context = BioContext::new_with_directories(
                config(false),
                ContextPolicy {
                    options: 0,
                    verify_flags: 0,
                    ciphers: None,
                    hostname_common_name: true,
                },
                KeyExchange {
                    dh_pem: None,
                    curve: None,
                },
                false,
                Tls13Settings {
                    post_handshake_auth: false,
                    num_tickets: 0,
                },
                vec![],
            )
            .unwrap();
            let pointer = context.retained_context().as_ptr();
            let mut client = context.wrap(Some("localhost".into())).unwrap();
            let mut server = server.wrap(None).unwrap();
            let directories = if directory {
                let hash = openssl::x509::X509::from_pem(&cert)
                    .unwrap()
                    .subject_name_hash();
                vec![TrustDirectory {
                    files: vec![TrustFile {
                        name: format!("{hash:08x}.0"),
                        data: cert.clone(),
                    }],
                }]
            } else {
                vec![]
            };
            context
                .update_trust((!directory).then_some(cert), false, directories)
                .unwrap();
            assert_eq!(context.retained_context().as_ptr(), pointer);
            assert_eq!(
                context.inspect_store(StoreQuery::Stats).unwrap().x509,
                u32::from(!directory)
            );
            // The active SSL owns the native store, lookup control and budget.
            // Lazy verification must remain valid after dropping the WIT owner.
            drop(context);
            handshake(&mut client, &mut server).unwrap();
            client.write(b"retained trust").unwrap();
            transfer(&mut client, &mut server);
            assert_eq!(server.read(100).unwrap(), b"retained trust");
        }
    }

    #[test]
    fn authenticates_after_handshake_with_retained_contexts() {
        use crate::bindings::ssl_bio::{ContextPolicy, KeyExchange, Tls13Settings};
        for mode in 0..4 {
            let (cert, key) = certificate();
            let mut cc = config(false);
            cc.ca_pem = Some(cert.clone());
            cc.minimum = TlsVersion::Tls13;
            if mode == 0 {
                cc.cert_pem = Some(cert.clone());
                cc.key_pem = Some(key.clone());
            }
            let mut sc = config(true);
            sc.minimum = TlsVersion::Tls13;
            sc.ca_pem = Some(cert.clone());
            sc.cert_pem = Some(cert);
            sc.key_pem = Some(key);
            sc.verify = if mode == 2 {
                VerifyMode::Required
            } else {
                VerifyMode::Optional
            };
            let make = |config, enabled| {
                BioContext::new_with_tls13(
                    config,
                    ContextPolicy {
                        options: 0,
                        verify_flags: 0,
                        ciphers: None,
                        hostname_common_name: true,
                    },
                    KeyExchange {
                        dh_pem: None,
                        curve: None,
                    },
                    false,
                    Tls13Settings {
                        post_handshake_auth: enabled,
                        num_tickets: 0,
                    },
                )
                .unwrap()
            };
            let mut client = make(cc, mode != 3).wrap(Some("localhost".into())).unwrap();
            let mut server = make(sc, true).wrap(None).unwrap();
            handshake(&mut client, &mut server).unwrap();
            assert!(server.peer_certificate().unwrap().is_none());
            assert!(client.verify_client_post_handshake().is_err());
            if mode == 3 {
                assert!(server.verify_client_post_handshake().is_err());
                continue;
            }
            server.verify_client_post_handshake().unwrap();
            assert!(server.drain(BUFFER_LIMIT as u32).is_empty());
            assert!(server.verify_client_post_handshake().is_err());
            server.write(b"challenge").unwrap();
            transfer(&mut server, &mut client);
            assert_eq!(client.read(16).unwrap(), b"challenge");
            client.write(b"response").unwrap();
            transfer(&mut client, &mut server);
            if mode == 2 {
                assert_eq!(server.read(16).unwrap_err().kind, ErrorKind::Failure);
                transfer(&mut server, &mut client);
                assert_eq!(client.read(16).unwrap_err().kind, ErrorKind::Failure);
            } else {
                assert_eq!(server.read(16).unwrap(), b"response");
                assert_eq!(server.peer_certificate().unwrap().is_some(), mode == 0);
                assert!(!client.session().unwrap().info().has_ticket);
            }
        }
    }
    #[test]
    fn rejects_unknown_ca_and_wrong_hostname() {
        for (trusted, name) in [(false, "localhost"), (true, "wrong.example")] {
            let (mut client, mut server) = pair(trusted, name);
            let error = handshake(&mut client, &mut server).unwrap_err();
            assert_eq!(error.kind, ErrorKind::Verification);
            assert!(error.verify_code.is_some());
        }
    }
    #[test]
    fn resumes_tls12_and_tls13_with_retained_contexts() {
        for maximum in [None, Some(TlsVersion::Tls12)] {
            let (cert, key) = certificate();
            let mut cc = config(false);
            cc.ca_pem = Some(cert.clone());
            cc.maximum = maximum;
            let mut sc = config(true);
            sc.cert_pem = Some(cert);
            sc.key_pem = Some(key);
            sc.maximum = maximum;
            let cc = BioContext::new(cc).unwrap();
            let sc = BioContext::new(sc).unwrap();
            let mut saved = None;
            for reused in [false, true] {
                let mut client = cc.wrap(Some("localhost".into())).unwrap();
                let mut server = sc.wrap(None).unwrap();
                if let Some(session) = &saved {
                    client.set_session(session).unwrap();
                }
                handshake(&mut client, &mut server).unwrap();
                assert_eq!(client.session_reused(), reused);
                assert_eq!(server.session_reused(), reused);
                server.write(b"ticket").unwrap();
                transfer(&mut server, &mut client);
                assert_eq!(client.read(6).unwrap(), b"ticket");
                let session = client.session().unwrap();
                assert!(session.info().has_ticket);
                saved = Some(session);
                let mut client_done = false;
                let mut server_done = false;
                for _ in 0..16 {
                    client_done |= client.shutdown().is_ok();
                    transfer(&mut client, &mut server);
                    server_done |= server.shutdown().is_ok();
                    transfer(&mut server, &mut client);
                    if client_done && server_done {
                        break;
                    }
                }
                assert!(client_done && server_done);
            }
            assert_eq!(cc.session_stats().connect_good, 2);
            assert_eq!(sc.session_stats().accept_good, 2);
            assert_eq!(cc.session_stats().hits, 1);
            assert_eq!(sc.session_stats().hits, 1);
            drop(cc);
            drop(sc);
            assert!(!saved.unwrap().info().id.is_empty());
        }
    }

    #[test]
    fn sessions_retain_context_and_reject_foreign_ssl() {
        let (mut client, mut server) = pair(true, "localhost");
        handshake(&mut client, &mut server).unwrap();
        let session = client.session().unwrap();
        assert!(client
            .set_session(&session)
            .unwrap_err()
            .message
            .contains("after handshake"));
        assert!(server
            .set_session(&session)
            .unwrap_err()
            .message
            .contains("server"));
        drop(client);
        drop(server);
        assert!(session.info().time > 0);
        let (mut other, _) = pair(true, "localhost");
        assert!(other
            .set_session(&session)
            .unwrap_err()
            .message
            .contains("different SSLContext"));
    }
    #[test]
    fn private_bio_allows_missing_name_without_disabling_chain_verification() {
        let (cert, key) = certificate();
        let mut server_settings = config(true);
        server_settings.cert_pem = Some(cert.clone());
        server_settings.key_pem = Some(key);
        let server_context = BioContext::new(server_settings).unwrap();
        let mut settings = config(false);
        settings.ca_pem = Some(cert);
        let context = BioContext::new(settings).unwrap();
        assert!(context.wrap_with_role(false, None).is_err());
        assert!(context
            .wrap_with_role(true, Some("localhost".into()))
            .is_err());
        let mut client = context.wrap_private_bio(false, None).unwrap();
        assert!(client
            .stream
            .ssl()
            .verify_mode()
            .contains(SslVerifyMode::PEER));
        let mut server = server_context.wrap(None).unwrap();
        handshake(&mut client, &mut server).unwrap();
        let untrusted = BioContext::new(config(false)).unwrap();
        let mut client = untrusted.wrap_private_bio(false, None).unwrap();
        let mut server = server_context.wrap(None).unwrap();
        assert_eq!(
            handshake(&mut client, &mut server).unwrap_err().kind,
            ErrorKind::Verification
        );
    }

    #[test]
    fn private_hostname_flags_preserve_unknown_bits() {
        let context = BioContext::new(config(false)).unwrap();
        for mask in [0, 4, 36, 0x8000_0024, u32::MAX] {
            // SAFETY: all native objects stay local; no concurrent SSL calls.
            unsafe { context.update_host_flags(mask) }.unwrap();
            let connection = context.wrap_private_bio(false, None).unwrap();
            let actual = unsafe {
                X509_VERIFY_PARAM_get_hostflags(openssl_sys::SSL_get0_param(
                    connection.stream.ssl().as_ptr(),
                ))
            };
            assert_eq!(actual, mask);
        }
    }

    #[test]
    fn private_hostname_flags_copy_into_each_ssl() {
        let cert = include_bytes!("../../../../tests/fixtures/ssl/common-name.crt").to_vec();
        let key = include_bytes!("../../../../tests/fixtures/ssl/server.key").to_vec();
        let mut cc = config(false);
        cc.ca_pem = Some(cert.clone());
        let mut sc = config(true);
        sc.cert_pem = Some(cert);
        sc.key_pem = Some(key);
        let context = BioContext::new(cc).unwrap();
        let server = BioContext::new(sc).unwrap();
        // SAFETY: test owns all contexts and SSLs with no concurrent operations.
        unsafe { context.update_host_flags(4) }.unwrap();
        let mut previous = context
            .wrap_private_bio(false, Some("localhost".into()))
            .unwrap();
        unsafe { context.update_host_flags(36) }.unwrap();
        let mut current = context
            .wrap_private_bio(false, Some("localhost".into()))
            .unwrap();
        handshake(&mut previous, &mut server.wrap(None).unwrap()).unwrap();
        assert_eq!(
            handshake(&mut current, &mut server.wrap(None).unwrap())
                .unwrap_err()
                .verify_code,
            Some(62)
        );
        // The public path still applies its explicit common-name policy.
        let mut public = context.wrap(Some("localhost".into())).unwrap();
        handshake(&mut public, &mut server.wrap(None).unwrap()).unwrap();
    }

    #[test]
    fn binary_alpn_copies_client_offer_and_keeps_server_preference_live() {
        let (cert, key) = certificate();
        let mut cc = config(false);
        cc.ca_pem = Some(cert.clone());
        let mut sc = config(true);
        sc.cert_pem = Some(cert);
        sc.key_pem = Some(key);
        let client = BioContext::new(cc).unwrap();
        let server = BioContext::new(sc).unwrap();
        for name in [vec![0], vec![255], b"h2".to_vec(), vec![b'x'; 255]] {
            let mut wire = vec![name.len() as u8];
            wire.extend_from_slice(&name);
            // SAFETY: test-owned contexts and SSLs, no concurrent native calls.
            unsafe { client.update_alpn_wire(wire.clone()) }.unwrap();
            let mut c = client
                .wrap_private_bio(false, Some("localhost".into()))
                .unwrap();
            let mut s = server.wrap(None).unwrap();
            assert!(c.selected_alpn_bytes().unwrap().is_none());
            unsafe {
                client.update_alpn_wire(vec![1, b'y']).unwrap();
                server.update_alpn_wire(wire).unwrap();
            }
            for invalid in [
                vec![0],
                vec![3, b'x'],
                vec![1, b'x', 0],
                vec![0; BUFFER_LIMIT + 1],
            ] {
                assert!(unsafe { server.update_alpn_wire(invalid) }.is_err());
            }
            handshake(&mut c, &mut s).unwrap();
            assert_eq!(c.selected_alpn_bytes().unwrap(), Some(name.clone()));
            assert_eq!(s.selected_alpn_bytes().unwrap(), Some(name));
        }
        unsafe { client.update_alpn_wire(Vec::new()) }.unwrap();
        let mut c = client
            .wrap_private_bio(false, Some("localhost".into()))
            .unwrap();
        let mut s = server.wrap(None).unwrap();
        handshake(&mut c, &mut s).unwrap();
        assert!(c.selected_alpn_bytes().unwrap().is_none());
    }

    #[test]
    fn certificate_resources_survive_connection_drop() {
        let (mut client, mut server) = pair(true, "localhost");
        assert!(client.certificate_chain(true).unwrap().is_none());
        assert!(client.certificate_chain(false).unwrap().is_none());
        handshake(&mut client, &mut server).unwrap();
        let wire = client.certificate_chain(false).unwrap().unwrap();
        let verified = client.certificate_chain(true).unwrap().unwrap();
        assert_eq!(wire.len(), 1);
        assert_eq!(verified.len(), 1);
        assert!(wire[0].equal(&verified[0]));
        assert!(server.certificate_chain(false).unwrap().is_none());
        drop(client);
        drop(server);
        let cert = &wire[0];
        let der = cert.public_bytes(2).unwrap();
        let pem = cert.public_bytes(1).unwrap();
        assert_eq!(
            openssl::x509::X509::from_pem(&pem)
                .unwrap()
                .to_der()
                .unwrap(),
            der
        );
        let aux = String::from_utf8(cert.public_bytes(0x101).unwrap()).unwrap();
        assert_eq!(
            aux.replace("TRUSTED CERTIFICATE", "CERTIFICATE").as_bytes(),
            pem
        );
        assert!(cert.public_bytes(0).is_err());
        assert!(cert.subject().unwrap().contains("localhost"));
        assert_eq!(cert.subject_hash(), verified[0].subject_hash());
        assert!(!cert.info().unwrap().subject.is_empty());
    }

    #[test]
    fn connection_metadata_survives_context_drop() {
        let (mut client, mut server) = pair(true, "localhost");
        assert!(client.compression().unwrap().is_none());
        assert!(server.compression().unwrap().is_none());
        assert!(client.channel_binding().unwrap().is_none());
        assert!(server.channel_binding().unwrap().is_none());
        assert!(client.shared_ciphers().unwrap().is_none());
        assert!(server.shared_ciphers().unwrap().is_none());
        handshake(&mut client, &mut server).unwrap();
        assert!(client.compression().unwrap().is_none());
        assert!(server.compression().unwrap().is_none());
        let binding = client.channel_binding().unwrap().unwrap();
        assert!(!binding.is_empty());
        assert_eq!(server.channel_binding().unwrap().as_ref(), Some(&binding));
        assert!(client.shared_ciphers().unwrap().is_none());
        let shared = server.shared_ciphers().unwrap().unwrap();
        let selected = server.cipher_info().unwrap().unwrap();
        assert!(shared.iter().any(|cipher| cipher.name == selected.name
            && cipher.protocol == selected.protocol
            && cipher.strength_bits == selected.strength_bits));
    }
    #[test]
    fn verifies_ip_san_without_sni() {
        let (mut client, mut server) = pair(true, "127.0.0.1");
        handshake(&mut client, &mut server).unwrap();
    }
    #[test]
    fn shutdown_consumes_buffered_peer_alert_without_prior_read() {
        let (mut client, mut server) = pair(true, "localhost");
        handshake(&mut client, &mut server).unwrap();
        assert_eq!(client.shutdown().unwrap_err().kind, ErrorKind::WantRead);
        transfer(&mut client, &mut server);
        server.shutdown().unwrap();
        transfer(&mut server, &mut client);
        client.shutdown().unwrap();
    }
    #[test]
    fn distinguishes_close_notify_from_truncated_transport() {
        let (mut client, mut server) = pair(true, "localhost");
        handshake(&mut client, &mut server).unwrap();
        assert_eq!(client.shutdown().unwrap_err().kind, ErrorKind::WantRead);
        transfer(&mut client, &mut server);
        assert_eq!(server.read(1).unwrap_err().kind, ErrorKind::ZeroReturn);
        server.shutdown().unwrap();
        transfer(&mut server, &mut client);
        client.shutdown().unwrap();
        let (mut client, mut server) = pair(true, "localhost");
        handshake(&mut client, &mut server).unwrap();
        client.feed_eof();
        assert_eq!(client.read(1).unwrap_err().kind, ErrorKind::Eof);
        assert!(client.feed(vec![1]).is_err());
    }
    #[test]
    fn native_bio_eof_waits_for_buffered_input_and_keeps_owned_state() {
        let (mut client, _) = pair(true, "localhost");
        // SAFETY: this test uniquely owns the live connection, no callback or
        // native operation is concurrent, and the BIO remains owned by stream.
        let bio = unsafe { openssl_sys::SSL_get_rbio(client.stream.ssl().as_ptr()) };
        let eof = || unsafe {
            openssl_sys::BIO_ctrl(bio, openssl_sys::BIO_CTRL_EOF, 0, std::ptr::null_mut())
        };
        assert_eq!(eof(), 0);
        client.feed(vec![1, 2, 3]).unwrap();
        client.feed_eof();
        assert_eq!(eof(), 0);
        let mut input = [0; 2];
        assert_eq!(client.stream.get_mut().read(&mut input).unwrap(), 2);
        assert_eq!(eof(), 0);
        assert_eq!(client.stream.get_mut().read(&mut input).unwrap(), 1);
        assert_eq!(eof(), 1);
        assert_eq!(client.stream.get_mut().read(&mut input).unwrap(), 0);
        assert_eq!(eof(), 1);
    }
    #[test]
    fn rejects_invalid_config_and_bounds_host_buffers() {
        let mut options = config(false);
        options.verify = VerifyMode::None;
        assert!(BioContext::new(options).is_err());
        let (mut client, _) = pair(true, "localhost");
        assert!(client.feed(vec![0; BUFFER_LIMIT + 1]).is_err());
        assert!(client.read(BUFFER_LIMIT as u32 + 1).is_err());
        assert!(client.write(&vec![0; BUFFER_LIMIT + 1]).is_err());
        let mut transport = MemoryTransport::default();
        assert_eq!(
            transport.write(&vec![0; BUFFER_LIMIT]).unwrap(),
            BUFFER_LIMIT
        );
        assert_eq!(
            transport.write(&[1]).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn rejects_encrypted_keys_without_prompting() {
        let (cert, key) = certificate();
        let key = PKey::private_key_from_pem(&key).unwrap();
        let encrypted = key
            .private_key_to_pem_pkcs8_passphrase(
                openssl::symm::Cipher::aes_256_cbc(),
                b"test-password",
            )
            .unwrap();
        let mut options = config(true);
        options.cert_pem = Some(cert);
        options.key_pem = Some(encrypted);
        assert!(BioContext::new(options).is_err());
    }

    #[test]
    fn unverified_certificate_does_not_mask_eof() {
        let (mut client, mut server) = pair_with_verify(false, "localhost", VerifyMode::None);
        handshake(&mut client, &mut server).unwrap();
        assert_ne!(client.stream.ssl().verify_result().as_raw(), 0);
        client.feed_eof();
        assert_eq!(client.read(1).unwrap_err().kind, ErrorKind::Eof);
    }

    #[test]
    fn peer_certificate_states() {
        let (mut client, mut server) = pair(true, "localhost");
        assert_eq!(
            client.peer_certificate().unwrap_err().kind,
            ErrorKind::InvalidInput
        );
        handshake(&mut client, &mut server).unwrap();
        assert!(matches!(
            client.peer_certificate().unwrap(),
            Some(PeerCertificate::Verified(_))
        ));
        assert!(server.peer_certificate().unwrap().is_none());
        let (mut client, mut server) = pair_with_verify(false, "localhost", VerifyMode::None);
        handshake(&mut client, &mut server).unwrap();
        assert!(matches!(
            client.peer_certificate().unwrap(),
            Some(PeerCertificate::Unverified)
        ));
    }
}
