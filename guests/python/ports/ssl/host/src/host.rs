//! Store-owned TLS resources. Operations run synchronously over memory only;
//! no lock is held across an await and no Tarantool API is called.
use crate::{
    bindings::ssl_bio::{
        self, CertificateInput, Config, ConnectionInfo, PeerCertificate, PreparedCredentials,
        TlsError,
    },
    tls_bio::{invalid, BioConnection, BioContext},
    tls_session::BioSession,
};
use wasmtime::component::Resource;

impl<T: crate::SslView> ssl_bio::Host for crate::SslHost<'_, T> {
    async fn default_ciphers(&mut self) -> String {
        crate::tls_policy::DEFAULT_CIPHERS.to_owned()
    }

    async fn compiled_api_version(&mut self) -> u64 {
        crate::tls_protocols::compiled_api_version()
    }

    async fn decode_certificate(
        &mut self,
        data: Vec<u8>,
    ) -> Result<ssl_bio::CertificateDetails, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        crate::tls_certificate::decode_pem(&data)
    }
    async fn verification_message(&mut self, code: i32) -> String {
        if let Err(error) = crate::openssl_dynamic::ensure_available() {
            return error.message;
        }

        crate::tls_errors::verification_message(code)
    }
    async fn describe_error(&mut self, message: String) -> ssl_bio::ErrorDetails {
        crate::tls_errors::details(&message)
    }
    async fn protocol_capabilities(&mut self) -> Vec<u16> {
        crate::tls_protocols::capabilities()
    }
    async fn prepare_policy_credentials(
        &mut self,
        cert: Vec<u8>,
        key: Vec<u8>,
        password: Option<Vec<u8>>,
        policy: ssl_bio::SeededPolicy,
    ) -> Result<ssl_bio::PreparedPolicyCredentials, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        crate::tls_credentials::prepare_policy_credentials(cert, key, password, policy)
    }
    async fn create_context_with_history(
        &mut self,
        options: Config,
        policy: ssl_bio::SeededPolicy,
        history: ssl_bio::ContextHistory,
        enabled: bool,
        tls13: ssl_bio::Tls13Settings,
        directories: Vec<ssl_bio::TrustDirectory>,
    ) -> Result<Resource<BioContext>, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        let context =
            BioContext::new_with_history(options, policy, history, enabled, tls13, directories)?;
        self.0
            .ssl_table()
            .push(context)
            .map_err(|e| invalid(e.to_string()))
    }
    async fn create_context_with_key_exchange_history(
        &mut self,
        options: Config,
        policy: ssl_bio::SeededPolicy,
        history: ssl_bio::KeyExchangeHistory,
        enabled: bool,
        tls13: ssl_bio::Tls13Settings,
        directories: Vec<ssl_bio::TrustDirectory>,
    ) -> Result<Resource<BioContext>, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        let context = BioContext::new_with_key_exchange_history(
            options,
            policy,
            history,
            enabled,
            tls13,
            directories,
        )?;
        self.0
            .ssl_table()
            .push(context)
            .map_err(|e| invalid(e.to_string()))
    }
    async fn create_context_with_seeded_policy(
        &mut self,
        options: Config,
        policy: ssl_bio::SeededPolicy,
        settings: ssl_bio::KeyExchange,
        enabled: bool,
        tls13: ssl_bio::Tls13Settings,
        directories: Vec<ssl_bio::TrustDirectory>,
    ) -> Result<Resource<BioContext>, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        let context = BioContext::new_with_seeded_policy(
            options,
            policy,
            settings,
            enabled,
            tls13,
            directories,
        )?;
        self.0
            .ssl_table()
            .push(context)
            .map_err(|e| invalid(e.to_string()))
    }
    async fn inspect_seeded_policy(
        &mut self,
        policy: ssl_bio::SeededPolicy,
    ) -> Result<ssl_bio::PolicyDetails, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        crate::tls_policy::inspect_seeded(policy)
    }
    async fn prepare_key_exchange_seeded(
        &mut self,
        settings: ssl_bio::KeyExchange,
        policy: ssl_bio::SeededPolicy,
    ) -> Result<ssl_bio::KeyExchange, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        crate::tls_key_exchange::prepare_seeded(settings, policy)
    }
    async fn create_context_with_directories(
        &mut self,
        options: Config,
        policy: ssl_bio::ContextPolicy,
        settings: ssl_bio::KeyExchange,
        enabled: bool,
        tls13: ssl_bio::Tls13Settings,
        directories: Vec<ssl_bio::TrustDirectory>,
    ) -> Result<Resource<BioContext>, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        let context = BioContext::new_with_directories(
            options,
            policy,
            settings,
            enabled,
            tls13,
            directories,
        )?;
        self.0
            .ssl_table()
            .push(context)
            .map_err(|e| invalid(e.to_string()))
    }
    async fn random_bytes(&mut self, size: u32) -> Result<Vec<u8>, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        crate::tls_utilities::random_bytes(size)
    }
    async fn random_status(&mut self) -> bool {
        crate::openssl_dynamic::ensure_available().is_ok() && crate::tls_utilities::random_status()
    }
    async fn random_add(&mut self, data: Vec<u8>, entropy: f64) -> Result<(), TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        crate::tls_utilities::random_add(&data, entropy)
    }
    async fn lookup_asn1_object(
        &mut self,
        query: ssl_bio::Asn1Query,
    ) -> Result<ssl_bio::Asn1ObjectInfo, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        crate::tls_utilities::lookup(query)
    }
    async fn default_verify_paths(&mut self) -> Result<ssl_bio::VerifyPathDefaults, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        crate::tls_utilities::default_verify_paths()
    }
    async fn create_context_with_tls13(
        &mut self,
        options: Config,
        policy: ssl_bio::ContextPolicy,
        settings: ssl_bio::KeyExchange,
        enabled: bool,
        tls13: ssl_bio::Tls13Settings,
    ) -> Result<Resource<BioContext>, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        let context = BioContext::new_with_tls13(options, policy, settings, enabled, tls13)?;
        self.0
            .ssl_table()
            .push(context)
            .map_err(|error| invalid(error.to_string()))
    }
    async fn create_context_with_sni(
        &mut self,
        options: Config,
        policy: ssl_bio::ContextPolicy,
        settings: ssl_bio::KeyExchange,
        enabled: bool,
    ) -> Result<Resource<BioContext>, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        let context = BioContext::new_with_sni(options, policy, settings, enabled)?;
        self.0
            .ssl_table()
            .push(context)
            .map_err(|error| invalid(error.to_string()))
    }
    async fn prepare_key_exchange(
        &mut self,
        settings: ssl_bio::KeyExchange,
        policy: ssl_bio::ContextPolicy,
    ) -> Result<ssl_bio::KeyExchange, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        crate::tls_key_exchange::prepare(settings, policy)
    }
    async fn create_context_with_key_exchange(
        &mut self,
        options: Config,
        policy: ssl_bio::ContextPolicy,
        settings: ssl_bio::KeyExchange,
    ) -> Result<Resource<BioContext>, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        let context = BioContext::new_with_key_exchange(options, policy, settings)?;
        self.0
            .ssl_table()
            .push(context)
            .map_err(|e| invalid(e.to_string()))
    }
    async fn backend(&mut self) -> ssl_bio::BackendInfo {
        if let Err(error) = crate::openssl_dynamic::ensure_available() {
            return ssl_bio::BackendInfo {
                version: error.message,
                number: 0,
            };
        }

        ssl_bio::BackendInfo {
            version: openssl::version::version().to_owned(),
            number: openssl::version::number() as u64,
        }
    }
    async fn prepare_store_pem(&mut self, data: Vec<u8>) -> Result<Vec<u8>, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        crate::tls_store::prepare(data)
    }
    async fn inspect_store(
        &mut self,
        data: Option<Vec<u8>>,
        default_certs: bool,
        query: ssl_bio::StoreQuery,
    ) -> Result<ssl_bio::StoreDetails, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        crate::tls_store::inspect(data, default_certs, query)
    }
    async fn inspect_store_with_default_sources(
        &mut self,
        data: Option<Vec<u8>>,
        sources: ssl_bio::DefaultTrustSources,
        query: ssl_bio::StoreQuery,
    ) -> Result<ssl_bio::StoreDetails, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        crate::tls_store::inspect_with_defaults(data, sources, query)
    }
    async fn inspect_policy(
        &mut self,
        policy: ssl_bio::ContextPolicy,
    ) -> Result<ssl_bio::PolicyDetails, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        crate::tls_policy::inspect(policy)
    }
    async fn create_context_with_policy(
        &mut self,
        options: Config,
        policy: ssl_bio::ContextPolicy,
    ) -> Result<Resource<BioContext>, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        let context = BioContext::new_with_policy(options, policy)?;
        self.0
            .ssl_table()
            .push(context)
            .map_err(|e| invalid(e.to_string()))
    }
    async fn prepare_ca(&mut self, data: CertificateInput) -> Result<Vec<u8>, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        crate::tls_credentials::prepare_ca(data)
    }
    async fn prepare_credentials(
        &mut self,
        cert: Vec<u8>,
        key: Vec<u8>,
        password: Option<Vec<u8>>,
    ) -> Result<PreparedCredentials, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        crate::tls_credentials::prepare_credentials(cert, key, password)
    }
    async fn create_context(&mut self, options: Config) -> Result<Resource<BioContext>, TlsError> {
        crate::openssl_dynamic::ensure_available()?;

        let context = BioContext::new(options)?;
        self.0
            .ssl_table()
            .push(context)
            .map_err(|e| invalid(e.to_string()))
    }
}
impl<T: crate::SslView> ssl_bio::HostContext for crate::SslHost<'_, T> {
    async fn update_credentials(
        &mut self,
        handle: Resource<BioContext>,
        value: ssl_bio::Credentials,
    ) -> Result<(), TlsError> {
        let mut state = self.0.ssl_table();
        let context = state.get_mut(&handle).map_err(|e| invalid(e.to_string()))?;
        // SAFETY: table lock excludes all associated native calls, no await.
        unsafe { context.update_credentials(value) }
    }
    async fn update_sni(
        &mut self,
        handle: Resource<BioContext>,
        enabled: bool,
    ) -> Result<(), TlsError> {
        let state = self.0.ssl_table();
        state
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .update_sni(enabled);
        Ok(())
    }
    async fn update_alpn(
        &mut self,
        handle: Resource<BioContext>,
        protocols: Vec<String>,
    ) -> Result<(), TlsError> {
        let state = self.0.ssl_table();
        let context = state.get(&handle).map_err(|e| invalid(e.to_string()))?;
        // SAFETY: same lock excludes every native SSL call; no await occurs.
        unsafe { context.update_alpn(protocols) }
    }
    async fn update_alpn_wire(
        &mut self,
        handle: Resource<BioContext>,
        protocols: Vec<u8>,
    ) -> Result<(), TlsError> {
        let state = self.0.ssl_table();
        let context = state.get(&handle).map_err(|e| invalid(e.to_string()))?;
        // SAFETY: table lock excludes every native SSL call; no await occurs.
        unsafe { context.update_alpn_wire(protocols) }
    }
    async fn update_host_flags(
        &mut self,
        handle: Resource<BioContext>,
        flags: u32,
    ) -> Result<(), TlsError> {
        let state = self.0.ssl_table();
        let context = state.get(&handle).map_err(|e| invalid(e.to_string()))?;
        // SAFETY: table lock excludes every native SSL call; no await occurs.
        unsafe { context.update_host_flags(flags) }
    }
    async fn update_key_exchange(
        &mut self,
        handle: Resource<BioContext>,
        settings: ssl_bio::KeyExchangeUpdate,
    ) -> Result<(), TlsError> {
        let state = self.0.ssl_table();
        let context = state.get(&handle).map_err(|e| invalid(e.to_string()))?;
        // SAFETY: this same table lock excludes all associated native calls;
        // no await or concurrent SSL operation occurs during the update.
        unsafe { context.update_key_exchange(settings) }
    }
    async fn update_tls13(
        &mut self,
        handle: Resource<BioContext>,
        settings: ssl_bio::Tls13Settings,
    ) -> Result<(), TlsError> {
        let mut state = self.0.ssl_table();
        let context = state.get_mut(&handle).map_err(|e| invalid(e.to_string()))?;
        // SAFETY: the same table lock serializes every native SSL operation;
        // no await or native operation occurs concurrently with this update.
        unsafe { context.update_tls13(settings) }
    }
    async fn update_settings(
        &mut self,
        handle: Resource<BioContext>,
        settings: ssl_bio::ConnectionSettings,
    ) -> Result<(), TlsError> {
        let mut state = self.0.ssl_table();
        let context = state.get_mut(&handle).map_err(|e| invalid(e.to_string()))?;
        // SAFETY: all associated native operations use the same table lock;
        // no await occurs under this lock and no SSL operation is in flight.
        unsafe { context.update_settings(settings) }
    }
    async fn update_protocol_settings(
        &mut self,
        handle: Resource<BioContext>,
        settings: ssl_bio::ProtocolSettings,
    ) -> Result<(), TlsError> {
        let mut state = self.0.ssl_table();
        let context = state.get_mut(&handle).map_err(|e| invalid(e.to_string()))?;
        // SAFETY: all native operations share this lock, no await under it.
        unsafe { context.update_protocol_settings(settings) }
    }
    async fn inspect_policy(
        &mut self,
        handle: Resource<BioContext>,
    ) -> Result<ssl_bio::PolicyDetails, TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .inspect_policy()
    }
    async fn update_policy(
        &mut self,
        handle: Resource<BioContext>,
        policy: ssl_bio::ContextPolicy,
    ) -> Result<(), TlsError> {
        let mut state = self.0.ssl_table();
        let context = state.get_mut(&handle).map_err(|e| invalid(e.to_string()))?;
        // SAFETY: this same lock serializes all context and connection native
        // operations; no SSL call is active, and no await occurs under it.
        unsafe { context.update_policy(policy) }
    }
    async fn update_trust(
        &mut self,
        handle: Resource<BioContext>,
        pem: Option<Vec<u8>>,
        default_certs: bool,
        directories: Vec<ssl_bio::TrustDirectory>,
    ) -> Result<(), TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .update_trust(pem, default_certs, directories)
    }
    async fn load_default_trust(
        &mut self,
        handle: Resource<BioContext>,
        sources: ssl_bio::DefaultTrustSources,
    ) -> Result<(), TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .load_default_trust(sources)
    }
    async fn inspect_store(
        &mut self,
        handle: Resource<BioContext>,
        query: ssl_bio::StoreQuery,
    ) -> Result<ssl_bio::StoreDetails, TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .inspect_store(query)
    }
    async fn session_stats(
        &mut self,
        handle: Resource<BioContext>,
    ) -> Result<ssl_bio::SessionStats, TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map(|context| context.session_stats())
            .map_err(|e| invalid(e.to_string()))
    }
    async fn wrap(
        &mut self,
        handle: Resource<BioContext>,
        server_name: Option<String>,
    ) -> Result<Resource<BioConnection>, TlsError> {
        let mut state = self.0.ssl_table();
        let connection = state
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .wrap(server_name)?;
        state.push(connection).map_err(|e| invalid(e.to_string()))
    }
    async fn wrap_private_bio(
        &mut self,
        handle: Resource<BioContext>,
        server_side: bool,
        server_name: Option<String>,
    ) -> Result<Resource<BioConnection>, TlsError> {
        let mut state = self.0.ssl_table();
        let connection = state
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .wrap_private_bio(server_side, server_name)?;
        state.push(connection).map_err(|e| invalid(e.to_string()))
    }
    async fn wrap_with_role(
        &mut self,
        handle: Resource<BioContext>,
        server_side: bool,
        server_name: Option<String>,
    ) -> Result<Resource<BioConnection>, TlsError> {
        let mut state = self.0.ssl_table();
        let connection = state
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .wrap_with_role(server_side, server_name)?;
        state.push(connection).map_err(|e| invalid(e.to_string()))
    }
    async fn drop(&mut self, handle: Resource<BioContext>) -> wasmtime::Result<()> {
        self.0.ssl_table().delete(handle)?;
        Ok(())
    }
}
impl<T: crate::SslView> ssl_bio::HostConnection for crate::SslHost<'_, T> {
    async fn messages_enabled(
        &mut self,
        handle: Resource<BioConnection>,
        enabled: bool,
    ) -> Result<(), TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .messages_enabled(enabled)
    }
    async fn take_messages(
        &mut self,
        handle: Resource<BioConnection>,
    ) -> Result<Vec<ssl_bio::TlsMessage>, TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .take_messages()
    }
    async fn keylog_enabled(
        &mut self,
        handle: Resource<BioConnection>,
        enabled: bool,
    ) -> Result<(), TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .keylog_enabled(enabled)
    }
    async fn take_keylog(&mut self, handle: Resource<BioConnection>) -> Result<Vec<u8>, TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .take_keylog()
    }
    async fn verify_client_post_handshake(
        &mut self,
        handle: Resource<BioConnection>,
    ) -> Result<(), TlsError> {
        self.0
            .ssl_table()
            .get_mut(&handle)
            .map_err(|error| invalid(error.to_string()))?
            .verify_client_post_handshake()
    }
    async fn pending_sni(
        &mut self,
        handle: Resource<BioConnection>,
    ) -> Result<Option<ssl_bio::SniRequest>, TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map_err(|error| invalid(error.to_string()))?
            .pending_sni()
    }
    async fn resolve_sni(
        &mut self,
        handle: Resource<BioConnection>,
        alert: Option<u8>,
    ) -> Result<(), TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map_err(|error| invalid(error.to_string()))?
            .resolve_sni(alert)
    }
    async fn set_context(
        &mut self,
        handle: Resource<BioConnection>,
        value: Resource<BioContext>,
    ) -> Result<(), TlsError> {
        let mut state = self.0.ssl_table();
        let context = state
            .get(&value)
            .map_err(|error| invalid(error.to_string()))?;
        let policy = context.sni_policy();
        let context = context.retained_context();
        state
            .get_mut(&handle)
            .map_err(|error| invalid(error.to_string()))?
            .set_context(&context, &policy)
    }
    async fn get_session(
        &mut self,
        handle: Resource<BioConnection>,
    ) -> Result<Option<Resource<BioSession>>, TlsError> {
        let mut state = self.0.ssl_table();
        let session = state
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .session();
        session
            .map(|session| state.push(session).map_err(|e| invalid(e.to_string())))
            .transpose()
    }
    async fn set_session(
        &mut self,
        handle: Resource<BioConnection>,
        session: Resource<BioSession>,
    ) -> Result<(), TlsError> {
        let mut state = self.0.ssl_table();
        let session = state
            .get(&session)
            .map_err(|e| invalid(e.to_string()))?
            .clone();
        state
            .get_mut(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .set_session(&session)
    }
    async fn session_reused(&mut self, handle: Resource<BioConnection>) -> Result<bool, TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map(|connection| connection.session_reused())
            .map_err(|e| invalid(e.to_string()))
    }
    async fn channel_binding(
        &mut self,
        handle: Resource<BioConnection>,
    ) -> Result<Option<Vec<u8>>, TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .channel_binding()
    }
    async fn selected_alpn_bytes(
        &mut self,
        handle: Resource<BioConnection>,
    ) -> Result<Option<Vec<u8>>, TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .selected_alpn_bytes()
    }
    async fn compression(
        &mut self,
        handle: Resource<BioConnection>,
    ) -> Result<Option<String>, TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .compression()
    }
    async fn shared_ciphers(
        &mut self,
        handle: Resource<BioConnection>,
    ) -> Result<Option<Vec<ssl_bio::SharedCipher>>, TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .shared_ciphers()
    }
    async fn cipher_info(
        &mut self,
        handle: Resource<BioConnection>,
    ) -> Result<Option<ssl_bio::CipherDetails>, TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .cipher_info()
    }
    async fn feed(
        &mut self,
        handle: Resource<BioConnection>,
        data: Vec<u8>,
    ) -> Result<(), TlsError> {
        self.0
            .ssl_table()
            .get_mut(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .feed(data)
    }
    async fn feed_eof(&mut self, handle: Resource<BioConnection>) -> Result<(), TlsError> {
        self.0
            .ssl_table()
            .get_mut(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .feed_eof();
        Ok(())
    }
    async fn drain(
        &mut self,
        handle: Resource<BioConnection>,
        size: u32,
    ) -> Result<Vec<u8>, TlsError> {
        self.0
            .ssl_table()
            .get_mut(&handle)
            .map(|c| c.drain(size))
            .map_err(|e| invalid(e.to_string()))
    }
    async fn handshake(&mut self, handle: Resource<BioConnection>) -> Result<(), TlsError> {
        self.0
            .ssl_table()
            .get_mut(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .handshake()
    }
    async fn read(
        &mut self,
        handle: Resource<BioConnection>,
        size: u32,
    ) -> Result<Vec<u8>, TlsError> {
        self.0
            .ssl_table()
            .get_mut(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .read(size)
    }
    async fn write(
        &mut self,
        handle: Resource<BioConnection>,
        data: Vec<u8>,
    ) -> Result<u32, TlsError> {
        self.0
            .ssl_table()
            .get_mut(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .write(&data)
    }
    async fn shutdown(&mut self, handle: Resource<BioConnection>) -> Result<(), TlsError> {
        self.0
            .ssl_table()
            .get_mut(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .shutdown()
    }
    async fn info(&mut self, handle: Resource<BioConnection>) -> Result<ConnectionInfo, TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .info()
    }
    async fn drop(&mut self, handle: Resource<BioConnection>) -> wasmtime::Result<()> {
        self.0.ssl_table().delete(handle)?;
        Ok(())
    }
    async fn peer_certificate(
        &mut self,
        handle: Resource<BioConnection>,
    ) -> Result<Option<PeerCertificate>, TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .peer_certificate()
    }
    async fn certificate_chain(
        &mut self,
        handle: Resource<BioConnection>,
        verified: bool,
    ) -> Result<Option<Vec<Resource<crate::tls_certificate::BioCertificate>>>, TlsError> {
        let mut state = self.0.ssl_table();
        let Some(certificates) = state
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .certificate_chain(verified)?
        else {
            return Ok(None);
        };
        let mut handles = Vec::new();
        for certificate in certificates {
            match state.push(certificate) {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    for handle in handles {
                        let _ = state.delete(handle);
                    }
                    return Err(invalid(error.to_string()));
                }
            }
        }
        Ok(Some(handles))
    }
}

impl<T: crate::SslView> ssl_bio::HostSession for crate::SslHost<'_, T> {
    async fn info(
        &mut self,
        handle: Resource<BioSession>,
    ) -> Result<ssl_bio::SessionInfo, TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map(|session| session.info())
            .map_err(|e| invalid(e.to_string()))
    }
    async fn drop(&mut self, handle: Resource<BioSession>) -> wasmtime::Result<()> {
        self.0.ssl_table().delete(handle)?;
        Ok(())
    }
}

impl<T: crate::SslView> ssl_bio::HostCertificate for crate::SslHost<'_, T> {
    async fn get_info(
        &mut self,
        handle: Resource<crate::tls_certificate::BioCertificate>,
    ) -> Result<ssl_bio::CertificateDetails, TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .info()
    }
    async fn public_bytes(
        &mut self,
        handle: Resource<crate::tls_certificate::BioCertificate>,
        format: u32,
    ) -> Result<Vec<u8>, TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .public_bytes(format)
    }
    async fn subject_hash(
        &mut self,
        handle: Resource<crate::tls_certificate::BioCertificate>,
    ) -> Result<u32, TlsError> {
        Ok(self
            .0
            .ssl_table()
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .subject_hash())
    }
    async fn subject(
        &mut self,
        handle: Resource<crate::tls_certificate::BioCertificate>,
    ) -> Result<String, TlsError> {
        self.0
            .ssl_table()
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .subject()
    }
    async fn equal(
        &mut self,
        handle: Resource<crate::tls_certificate::BioCertificate>,
        other: Resource<crate::tls_certificate::BioCertificate>,
    ) -> Result<bool, TlsError> {
        let state = self.0.ssl_table();
        Ok(state
            .get(&handle)
            .map_err(|e| invalid(e.to_string()))?
            .equal(state.get(&other).map_err(|e| invalid(e.to_string()))?))
    }
    async fn drop(
        &mut self,
        handle: Resource<crate::tls_certificate::BioCertificate>,
    ) -> wasmtime::Result<()> {
        self.0.ssl_table().delete(handle)?;
        Ok(())
    }
}
