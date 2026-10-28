/* OpenSSL entry points used by the SSL host and its native tests.
 * Keep forwarding signatures checked against the build headers. */
TWR_VOID(ACCESS_DESCRIPTION_free, (ACCESS_DESCRIPTION * a0),
    (a0))
TWR_FUNCTION(BIGNUM *, ASN1_INTEGER_to_BN, (const ASN1_INTEGER * a0, BIGNUM * a1),
    (a0, a1))
TWR_VOID(ASN1_OBJECT_free, (ASN1_OBJECT * a0),
    (a0))
TWR_FUNCTION(const unsigned char *, ASN1_STRING_get0_data, (const ASN1_STRING * a0),
    (a0))
TWR_FUNCTION(int, ASN1_STRING_length, (const ASN1_STRING * a0),
    (a0))
TWR_FUNCTION(int, ASN1_STRING_set, (ASN1_STRING * a0, const void * a1, int a2),
    (a0, a1, a2))
TWR_FUNCTION(int, ASN1_STRING_to_UTF8, (unsigned char ** a0, const ASN1_STRING * a1),
    (a0, a1))
TWR_FUNCTION(int, ASN1_STRING_type, (const ASN1_STRING * a0),
    (a0))
TWR_FUNCTION(ASN1_STRING *, ASN1_STRING_type_new, (int a0),
    (a0))
TWR_VOID(ASN1_TIME_free, (ASN1_TIME * a0),
    (a0))
TWR_FUNCTION(int, ASN1_TIME_print, (BIO * a0, const ASN1_TIME * a1),
    (a0, a1))
TWR_VOID(BIO_clear_flags, (BIO * a0, int a1),
    (a0, a1))
TWR_FUNCTION(long, BIO_ctrl, (BIO * a0, int a1, long a2, void * a3),
    (a0, a1, a2, a3))
TWR_VOID(BIO_free_all, (BIO * a0),
    (a0))
TWR_FUNCTION(char *, BIO_get_callback_arg, (const BIO * a0),
    (a0))
TWR_FUNCTION(void *, BIO_get_data, (BIO * a0),
    (a0))
TWR_FUNCTION(int, BIO_gets, (BIO * a0, char * a1, int a2),
    (a0, a1, a2))
TWR_VOID(BIO_meth_free, (BIO_METHOD * a0),
    (a0))
TWR_FUNCTION(BIO_METHOD *, BIO_meth_new, (int a0, const char * a1),
    (a0, a1))
TWR_FUNCTION(int, BIO_meth_set_create, (BIO_METHOD * a0, int (*a1)(BIO *)),
    (a0, a1))
TWR_FUNCTION(int, BIO_meth_set_ctrl, (BIO_METHOD * a0, long (*a1)(BIO *, int, long, void *)),
    (a0, a1))
TWR_FUNCTION(int, BIO_meth_set_destroy, (BIO_METHOD * a0, int (*a1)(BIO *)),
    (a0, a1))
TWR_FUNCTION(int, BIO_meth_set_puts, (BIO_METHOD * a0, int (*a1)(BIO *, const char *)),
    (a0, a1))
TWR_FUNCTION(int, BIO_meth_set_read, (BIO_METHOD * a0, int (*a1)(BIO *, char *, int)),
    (a0, a1))
TWR_FUNCTION(int, BIO_meth_set_write, (BIO_METHOD * a0, int (*a1)(BIO *, const char *, int)),
    (a0, a1))
TWR_FUNCTION(BIO *, BIO_new, (const BIO_METHOD * a0),
    (a0))
TWR_FUNCTION(BIO *, BIO_new_mem_buf, (const void * a0, int a1),
    (a0, a1))
TWR_FUNCTION(int, BIO_read, (BIO * a0, void * a1, int a2),
    (a0, a1, a2))
TWR_FUNCTION(const BIO_METHOD *, BIO_s_mem, (void),
    ())
TWR_VOID(BIO_set_callback_arg, (BIO * a0, char * a1),
    (a0, a1))
TWR_VOID(BIO_set_callback_ex, (BIO * a0, long (*a1)(BIO *, int, const char *, size_t, int, long, int, size_t *)),
    (a0, a1))
TWR_VOID(BIO_set_data, (BIO * a0, void * a1),
    (a0, a1))
TWR_VOID(BIO_set_flags, (BIO * a0, int a1),
    (a0, a1))
TWR_VOID(BIO_set_init, (BIO * a0, int a1),
    (a0, a1))
TWR_FUNCTION(char *, BN_bn2hex, (const BIGNUM * a0),
    (a0))
TWR_VOID(BN_free, (BIGNUM * a0),
    (a0))
TWR_FUNCTION(BIGNUM *, BN_new, (void),
    ())
TWR_FUNCTION(int, BN_set_word, (BIGNUM * a0, unsigned long a1),
    (a0, a1))
TWR_VOID(CRYPTO_free, (void * a0, const char * a1, int a2),
    (a0, a1, a2))
TWR_FUNCTION(int, CRYPTO_get_ex_new_index, (int a0, long a1, void * a2, CRYPTO_EX_new * a3, CRYPTO_EX_dup * a4, CRYPTO_EX_free * a5),
    (a0, a1, a2, a3, a4, a5))
TWR_FUNCTION(void *, CRYPTO_malloc, (size_t a0, const char * a1, int a2),
    (a0, a1, a2))
TWR_VOID(DH_free, (DH * a0),
    (a0))
TWR_VOID(DIST_POINT_free, (DIST_POINT * a0),
    (a0))
TWR_VOID(ERR_clear_error, (void),
    ())
TWR_FUNCTION(unsigned long, ERR_get_error_all, (const char ** a0, int * a1, const char ** a2, const char ** a3, int * a4),
    (a0, a1, a2, a3, a4))
TWR_FUNCTION(const char *, ERR_lib_error_string, (unsigned long a0),
    (a0))
TWR_VOID(ERR_new, (void),
    ())
TWR_FUNCTION(const char *, ERR_reason_error_string, (unsigned long a0),
    (a0))
TWR_VOID(ERR_set_debug, (const char * a0, int a1, const char * a2),
    (a0, a1, a2))
TWR_VOID(ERR_set_error_data, (char * a0, int a1),
    (a0, a1))
TWR_VOID(ERR_vset_error, (int a0, int a1, const char * a2, va_list a3),
    (a0, a1, a2, a3))
TWR_FUNCTION(int, EVP_PKEY_cmp, (const EVP_PKEY * a0, const EVP_PKEY * a1),
    (a0, a1))
TWR_VOID(EVP_PKEY_free, (EVP_PKEY * a0),
    (a0))
TWR_FUNCTION(int, EVP_PKEY_get_id, (const EVP_PKEY * a0),
    (a0))
TWR_FUNCTION(EVP_PKEY *, EVP_PKEY_new, (void),
    ())
TWR_FUNCTION(int, EVP_PKEY_set1_RSA, (EVP_PKEY * a0, struct rsa_st * a1),
    (a0, a1))
TWR_FUNCTION(int, EVP_Q_digest, (OSSL_LIB_CTX * a0, const char * a1, const char * a2, const void * a3, size_t a4, unsigned char * a5, size_t * a6),
    (a0, a1, a2, a3, a4, a5, a6))
TWR_FUNCTION(const EVP_CIPHER *, EVP_aes_256_cbc, (void),
    ())
TWR_FUNCTION(const EVP_MD *, EVP_sha256, (void),
    ())
TWR_VOID(GENERAL_NAME_free, (GENERAL_NAME * a0),
    (a0))
TWR_FUNCTION(GENERAL_NAME *, GENERAL_NAME_new, (void),
    ())
TWR_FUNCTION(int, GENERAL_NAME_print, (BIO * a0, GENERAL_NAME * a1),
    (a0, a1))
TWR_FUNCTION(int, GENERAL_NAME_set0_othername, (GENERAL_NAME * a0, ASN1_OBJECT * a1, ASN1_TYPE * a2),
    (a0, a1, a2))
TWR_FUNCTION(ASN1_OBJECT *, OBJ_dup, (const ASN1_OBJECT * a0),
    (a0))
TWR_FUNCTION(const char *, OBJ_nid2ln, (int a0),
    (a0))
TWR_FUNCTION(ASN1_OBJECT *, OBJ_nid2obj, (int a0),
    (a0))
TWR_FUNCTION(const char *, OBJ_nid2sn, (int a0),
    (a0))
TWR_FUNCTION(int, OBJ_obj2nid, (const ASN1_OBJECT * a0),
    (a0))
TWR_FUNCTION(int, OBJ_obj2txt, (char * a0, int a1, const ASN1_OBJECT * a2, int a3),
    (a0, a1, a2, a3))
TWR_FUNCTION(int, OBJ_sn2nid, (const char * a0),
    (a0))
TWR_FUNCTION(ASN1_OBJECT *, OBJ_txt2obj, (const char * a0, int a1),
    (a0, a1))
TWR_FUNCTION(int, OPENSSL_init_ssl, (uint64_t a0, const OPENSSL_INIT_SETTINGS * a1),
    (a0, a1))
TWR_VOID(OPENSSL_sk_free, (OPENSSL_STACK * a0),
    (a0))
TWR_FUNCTION(OPENSSL_STACK *, OPENSSL_sk_new_null, (void),
    ())
TWR_FUNCTION(int, OPENSSL_sk_num, (const OPENSSL_STACK * a0),
    (a0))
TWR_FUNCTION(void *, OPENSSL_sk_pop, (OPENSSL_STACK * a0),
    (a0))
TWR_FUNCTION(int, OPENSSL_sk_push, (OPENSSL_STACK * a0, const void * a1),
    (a0, a1))
TWR_FUNCTION(void *, OPENSSL_sk_value, (const OPENSSL_STACK * a0, int a1),
    (a0, a1))
TWR_FUNCTION(const char *, OpenSSL_version, (int a0),
    (a0))
TWR_FUNCTION(unsigned long, OpenSSL_version_num, (void),
    ())
TWR_FUNCTION(DH *, PEM_read_bio_DHparams, (BIO * a0, DH ** a1, pem_password_cb * a2, void * a3),
    (a0, a1, a2, a3))
TWR_FUNCTION(EVP_PKEY *, PEM_read_bio_PrivateKey, (BIO * a0, EVP_PKEY ** a1, pem_password_cb * a2, void * a3),
    (a0, a1, a2, a3))
TWR_FUNCTION(X509 *, PEM_read_bio_X509, (BIO * a0, X509 ** a1, pem_password_cb * a2, void * a3),
    (a0, a1, a2, a3))
TWR_FUNCTION(X509 *, PEM_read_bio_X509_AUX, (BIO * a0, X509 ** a1, pem_password_cb * a2, void * a3),
    (a0, a1, a2, a3))
TWR_FUNCTION(X509_CRL *, PEM_read_bio_X509_CRL, (BIO * a0, X509_CRL ** a1, pem_password_cb * a2, void * a3),
    (a0, a1, a2, a3))
TWR_FUNCTION(int, PEM_write_bio_DHparams, (BIO * a0, const DH * a1),
    (a0, a1))
TWR_FUNCTION(int, PEM_write_bio_PKCS8PrivateKey, (BIO * a0, const EVP_PKEY * a1, const EVP_CIPHER * a2, const char * a3, int a4, pem_password_cb * a5, void * a6),
    (a0, a1, a2, a3, a4, a5, a6))
TWR_FUNCTION(int, PEM_write_bio_X509, (BIO * a0, const X509 * a1),
    (a0, a1))
TWR_FUNCTION(int, PEM_write_bio_X509_AUX, (BIO * a0, const X509 * a1),
    (a0, a1))
TWR_VOID(RAND_add, (const void * a0, int a1, double a2),
    (a0, a1, a2))
TWR_FUNCTION(int, RAND_bytes, (unsigned char * a0, int a1),
    (a0, a1))
TWR_FUNCTION(int, RAND_status, (void),
    ())
TWR_VOID(RSA_free, (RSA * a0),
    (a0))
TWR_FUNCTION(int, RSA_generate_key_ex, (RSA * a0, int a1, BIGNUM * a2, BN_GENCB * a3),
    (a0, a1, a2, a3))
TWR_FUNCTION(RSA *, RSA_new, (void),
    ())
TWR_FUNCTION(char *, SSL_CIPHER_description, (const SSL_CIPHER * a0, char * a1, int a2),
    (a0, a1, a2))
TWR_FUNCTION(int, SSL_CIPHER_get_auth_nid, (const SSL_CIPHER * a0),
    (a0))
TWR_FUNCTION(int, SSL_CIPHER_get_bits, (const SSL_CIPHER * a0, int * a1),
    (a0, a1))
TWR_FUNCTION(int, SSL_CIPHER_get_cipher_nid, (const SSL_CIPHER * a0),
    (a0))
TWR_FUNCTION(int, SSL_CIPHER_get_digest_nid, (const SSL_CIPHER * a0),
    (a0))
TWR_FUNCTION(uint32_t, SSL_CIPHER_get_id, (const SSL_CIPHER * a0),
    (a0))
TWR_FUNCTION(int, SSL_CIPHER_get_kx_nid, (const SSL_CIPHER * a0),
    (a0))
TWR_FUNCTION(const char *, SSL_CIPHER_get_name, (const SSL_CIPHER * a0),
    (a0))
TWR_FUNCTION(const char *, SSL_CIPHER_get_version, (const SSL_CIPHER * a0),
    (a0))
TWR_FUNCTION(int, SSL_CIPHER_is_aead, (const SSL_CIPHER * a0),
    (a0))
TWR_FUNCTION(const char *, SSL_COMP_get_name, (const COMP_METHOD * a0),
    (a0))
TWR_FUNCTION(long, SSL_CTX_callback_ctrl, (SSL_CTX * a0, int a1, void (*a2)(void)),
    (a0, a1, a2))
TWR_FUNCTION(uint64_t, SSL_CTX_clear_options, (SSL_CTX * a0, uint64_t a1),
    (a0, a1))
TWR_FUNCTION(long, SSL_CTX_ctrl, (SSL_CTX * a0, int a1, long a2, void * a3),
    (a0, a1, a2, a3))
TWR_VOID(SSL_CTX_free, (SSL_CTX * a0),
    (a0))
TWR_FUNCTION(X509_VERIFY_PARAM *, SSL_CTX_get0_param, (SSL_CTX * a0),
    (a0))
TWR_FUNCTION(X509_STORE *, SSL_CTX_get_cert_store, (const SSL_CTX * a0),
    (a0))
TWR_FUNCTION(void *, SSL_CTX_get_ex_data, (const SSL_CTX * a0, int a1),
    (a0, a1))
TWR_FUNCTION(size_t, SSL_CTX_get_num_tickets, (const SSL_CTX * a0),
    (a0))
TWR_FUNCTION(uint64_t, SSL_CTX_get_options, (const SSL_CTX * a0),
    (a0))
TWR_FUNCTION(int, SSL_CTX_get_security_level, (const SSL_CTX * a0),
    (a0))
TWR_FUNCTION(int, SSL_CTX_get_verify_mode, (const SSL_CTX * a0),
    (a0))
TWR_FUNCTION(SSL_CTX *, SSL_CTX_new, (const SSL_METHOD * a0),
    (a0))
TWR_FUNCTION(int, SSL_CTX_set_alpn_protos, (SSL_CTX * a0, const unsigned char * a1, unsigned int a2),
    (a0, a1, a2))
TWR_VOID(SSL_CTX_set_alpn_select_cb, (SSL_CTX * a0, int (*a1)(SSL *, const unsigned char **, unsigned char *, const unsigned char *, unsigned int, void *), void * a2),
    (a0, a1, a2))
TWR_FUNCTION(int, SSL_CTX_set_cipher_list, (SSL_CTX * a0, const char * a1),
    (a0, a1))
TWR_VOID(SSL_CTX_set_client_hello_cb, (SSL_CTX * a0, int (*a1)(SSL *, int *, void *), void * a2),
    (a0, a1, a2))
TWR_FUNCTION(int, SSL_CTX_set_default_verify_dir, (SSL_CTX * a0),
    (a0))
TWR_FUNCTION(int, SSL_CTX_set_default_verify_file, (SSL_CTX * a0),
    (a0))
TWR_FUNCTION(int, SSL_CTX_set_default_verify_paths, (SSL_CTX * a0),
    (a0))
TWR_FUNCTION(int, SSL_CTX_set_ex_data, (SSL_CTX * a0, int a1, void * a2),
    (a0, a1, a2))
TWR_VOID(SSL_CTX_set_keylog_callback, (SSL_CTX * a0, void (*a1)(const SSL *, const char *)),
    (a0, a1))
TWR_FUNCTION(int, SSL_CTX_set_num_tickets, (SSL_CTX * a0, size_t a1),
    (a0, a1))
TWR_FUNCTION(uint64_t, SSL_CTX_set_options, (SSL_CTX * a0, uint64_t a1),
    (a0, a1))
TWR_VOID(SSL_CTX_set_post_handshake_auth, (SSL_CTX * a0, int a1),
    (a0, a1))
TWR_VOID(SSL_CTX_set_security_level, (SSL_CTX * a0, int a1),
    (a0, a1))
TWR_FUNCTION(int, SSL_CTX_set_session_id_context, (SSL_CTX * a0, const unsigned char * a1, unsigned int a2),
    (a0, a1, a2))
TWR_VOID(SSL_CTX_set_verify, (SSL_CTX * a0, int a1, int (*a2)(int, X509_STORE_CTX *)),
    (a0, a1, a2))
TWR_FUNCTION(int, SSL_CTX_up_ref, (SSL_CTX * a0),
    (a0))
TWR_FUNCTION(int, SSL_CTX_use_cert_and_key, (SSL_CTX * a0, X509 * a1, EVP_PKEY * a2, struct stack_st_X509 * a3, int a4),
    (a0, a1, a2, a3, a4))
TWR_FUNCTION(int, SSL_CTX_use_certificate, (SSL_CTX * a0, X509 * a1),
    (a0, a1))
TWR_VOID(SSL_SESSION_free, (SSL_SESSION * a0),
    (a0))
TWR_FUNCTION(const unsigned char *, SSL_SESSION_get_id, (const SSL_SESSION * a0, unsigned int * a1),
    (a0, a1))
TWR_FUNCTION(unsigned long, SSL_SESSION_get_ticket_lifetime_hint, (const SSL_SESSION * a0),
    (a0))
TWR_FUNCTION(long, SSL_SESSION_get_time, (const SSL_SESSION * a0),
    (a0))
TWR_FUNCTION(long, SSL_SESSION_get_timeout, (const SSL_SESSION * a0),
    (a0))
TWR_FUNCTION(int, SSL_SESSION_has_ticket, (const SSL_SESSION * a0),
    (a0))
TWR_FUNCTION(int, SSL_SESSION_up_ref, (SSL_SESSION * a0),
    (a0))
TWR_FUNCTION(int, SSL_client_hello_get0_ext, (SSL * a0, unsigned int a1, const unsigned char ** a2, size_t * a3),
    (a0, a1, a2, a3))
TWR_FUNCTION(long, SSL_ctrl, (SSL * a0, int a1, long a2, void * a3),
    (a0, a1, a2, a3))
TWR_FUNCTION(int, SSL_do_handshake, (SSL * a0),
    (a0))
TWR_VOID(SSL_free, (SSL * a0),
    (a0))
TWR_VOID(SSL_get0_alpn_selected, (const SSL * a0, const unsigned char ** a1, unsigned int * a2),
    (a0, a1, a2))
TWR_FUNCTION(X509_VERIFY_PARAM *, SSL_get0_param, (SSL * a0),
    (a0))
TWR_FUNCTION(struct stack_st_X509 *, SSL_get0_verified_chain, (const SSL * a0),
    (a0))
TWR_FUNCTION(X509 *, SSL_get1_peer_certificate, (const SSL * a0),
    (a0))
TWR_FUNCTION(SSL_CTX *, SSL_get_SSL_CTX, (const SSL * a0),
    (a0))
TWR_FUNCTION(X509 *, SSL_get_certificate, (const SSL * a0),
    (a0))
TWR_FUNCTION(struct stack_st_SSL_CIPHER *, SSL_get_ciphers, (const SSL * a0),
    (a0))
TWR_FUNCTION(struct stack_st_SSL_CIPHER *, SSL_get_client_ciphers, (const SSL * a0),
    (a0))
TWR_FUNCTION(const SSL_CIPHER *, SSL_get_current_cipher, (const SSL * a0),
    (a0))
TWR_FUNCTION(const COMP_METHOD *, SSL_get_current_compression, (const SSL * a0),
    (a0))
TWR_FUNCTION(int, SSL_get_error, (const SSL * a0, int a1),
    (a0, a1))
TWR_FUNCTION(void *, SSL_get_ex_data, (const SSL * a0, int a1),
    (a0, a1))
TWR_FUNCTION(size_t, SSL_get_finished, (const SSL * a0, void * a1, size_t a2),
    (a0, a1, a2))
TWR_FUNCTION(size_t, SSL_get_num_tickets, (const SSL * a0),
    (a0))
TWR_FUNCTION(struct stack_st_X509 *, SSL_get_peer_cert_chain, (const SSL * a0),
    (a0))
TWR_FUNCTION(size_t, SSL_get_peer_finished, (const SSL * a0, void * a1, size_t a2),
    (a0, a1, a2))
TWR_FUNCTION(BIO *, SSL_get_rbio, (const SSL * a0),
    (a0))
TWR_FUNCTION(SSL_SESSION *, SSL_get_session, (const SSL * a0),
    (a0))
TWR_FUNCTION(int, SSL_get_shutdown, (const SSL * a0),
    (a0))
TWR_FUNCTION(int, SSL_get_verify_mode, (const SSL * a0),
    (a0))
TWR_FUNCTION(long, SSL_get_verify_result, (const SSL * a0),
    (a0))
TWR_FUNCTION(const char *, SSL_get_version, (const SSL * a0),
    (a0))
TWR_FUNCTION(int, SSL_is_init_finished, (const SSL * a0),
    (a0))
TWR_FUNCTION(int, SSL_is_server, (const SSL * a0),
    (a0))
TWR_FUNCTION(SSL *, SSL_new, (SSL_CTX * a0),
    (a0))
TWR_FUNCTION(int, SSL_pending, (const SSL * a0),
    (a0))
TWR_FUNCTION(int, SSL_read_ex, (SSL * a0, void * a1, size_t a2, size_t * a3),
    (a0, a1, a2, a3))
TWR_FUNCTION(int, SSL_session_reused, (const SSL * a0),
    (a0))
TWR_FUNCTION(SSL_CTX *, SSL_set_SSL_CTX, (SSL * a0, SSL_CTX * a1),
    (a0, a1))
TWR_VOID(SSL_set_accept_state, (SSL * a0),
    (a0))
TWR_VOID(SSL_set_bio, (SSL * a0, BIO * a1, BIO * a2),
    (a0, a1, a2))
TWR_VOID(SSL_set_connect_state, (SSL * a0),
    (a0))
TWR_FUNCTION(int, SSL_set_ex_data, (SSL * a0, int a1, void * a2),
    (a0, a1, a2))
TWR_VOID(SSL_set_msg_callback, (SSL * a0, void (*a1)(int, int, int, const void *, size_t, SSL *, void *)),
    (a0, a1))
TWR_VOID(SSL_set_post_handshake_auth, (SSL * a0, int a1),
    (a0, a1))
TWR_FUNCTION(int, SSL_set_session, (SSL * a0, SSL_SESSION * a1),
    (a0, a1))
TWR_VOID(SSL_set_shutdown, (SSL * a0, int a1),
    (a0, a1))
TWR_VOID(SSL_set_verify, (SSL * a0, int a1, int (*a2)(int, X509_STORE_CTX *)),
    (a0, a1, a2))
TWR_FUNCTION(int, SSL_shutdown, (SSL * a0),
    (a0))
TWR_FUNCTION(int, SSL_verify_client_post_handshake, (SSL * a0),
    (a0))
TWR_FUNCTION(int, SSL_write_ex, (SSL * a0, const void * a1, size_t a2, size_t * a3),
    (a0, a1, a2, a3))
TWR_FUNCTION(const SSL_METHOD *, TLS_method, (void),
    ())
TWR_FUNCTION(X509_EXTENSION *, X509V3_EXT_i2d, (int a0, int a1, void * a2),
    (a0, a1, a2))
TWR_FUNCTION(X509_EXTENSION *, X509V3_EXT_nconf_nid, (CONF * a0, X509V3_CTX * a1, int a2, const char * a3),
    (a0, a1, a2, a3))
TWR_VOID(X509V3_set_ctx, (X509V3_CTX * a0, X509 * a1, X509 * a2, X509_REQ * a3, X509_CRL * a4, int a5),
    (a0, a1, a2, a3, a4, a5))
TWR_VOID(X509V3_set_nconf, (X509V3_CTX * a0, CONF * a1),
    (a0, a1))
TWR_VOID(X509_ALGOR_get0, (const ASN1_OBJECT ** a0, int * a1, const void ** a2, const X509_ALGOR * a3),
    (a0, a1, a2, a3))
TWR_VOID(X509_CRL_free, (X509_CRL * a0),
    (a0))
TWR_VOID(X509_EXTENSION_free, (X509_EXTENSION * a0),
    (a0))
TWR_FUNCTION(void *, X509_LOOKUP_get_method_data, (const X509_LOOKUP * a0),
    (a0))
TWR_FUNCTION(X509_STORE *, X509_LOOKUP_get_store, (const X509_LOOKUP * a0),
    (a0))
TWR_VOID(X509_LOOKUP_meth_free, (X509_LOOKUP_METHOD * a0),
    (a0))
TWR_FUNCTION(X509_LOOKUP_METHOD *, X509_LOOKUP_meth_new, (const char * a0),
    (a0))
TWR_FUNCTION(int, X509_LOOKUP_meth_set_free, (X509_LOOKUP_METHOD * a0, void (*a1)(X509_LOOKUP *)),
    (a0, a1))
TWR_FUNCTION(int, X509_LOOKUP_meth_set_get_by_subject, (X509_LOOKUP_METHOD * a0, int (*a1)(X509_LOOKUP *, X509_LOOKUP_TYPE, const X509_NAME *, X509_OBJECT *)),
    (a0, a1))
TWR_FUNCTION(int, X509_LOOKUP_set_method_data, (X509_LOOKUP * a0, void * a1),
    (a0, a1))
TWR_FUNCTION(ASN1_STRING *, X509_NAME_ENTRY_get_data, (const X509_NAME_ENTRY * a0),
    (a0))
TWR_FUNCTION(ASN1_OBJECT *, X509_NAME_ENTRY_get_object, (const X509_NAME_ENTRY * a0),
    (a0))
TWR_FUNCTION(int, X509_NAME_ENTRY_set, (const X509_NAME_ENTRY * a0),
    (a0))
TWR_FUNCTION(int, X509_NAME_add_entry_by_txt, (X509_NAME * a0, const char * a1, int a2, const unsigned char * a3, int a4, int a5, int a6),
    (a0, a1, a2, a3, a4, a5, a6))
TWR_FUNCTION(X509_NAME *, X509_NAME_dup, (const X509_NAME * a0),
    (a0))
TWR_FUNCTION(int, X509_NAME_entry_count, (const X509_NAME * a0),
    (a0))
TWR_VOID(X509_NAME_free, (X509_NAME * a0),
    (a0))
TWR_FUNCTION(X509_NAME_ENTRY *, X509_NAME_get_entry, (const X509_NAME * a0, int a1),
    (a0, a1))
TWR_FUNCTION(int, X509_NAME_get_index_by_NID, (const X509_NAME * a0, int a1, int a2),
    (a0, a1, a2))
TWR_FUNCTION(X509_NAME *, X509_NAME_new, (void),
    ())
TWR_FUNCTION(int, X509_NAME_print_ex, (BIO * a0, const X509_NAME * a1, int a2, unsigned long a3),
    (a0, a1, a2, a3))
TWR_FUNCTION(X509 *, X509_OBJECT_get0_X509, (const X509_OBJECT * a0),
    (a0))
TWR_FUNCTION(X509_CRL *, X509_OBJECT_get0_X509_CRL, (const X509_OBJECT * a0),
    (a0))
TWR_FUNCTION(X509_LOOKUP_TYPE, X509_OBJECT_get_type, (const X509_OBJECT * a0),
    (a0))
TWR_FUNCTION(X509_OBJECT *, X509_OBJECT_retrieve_by_subject, (struct stack_st_X509_OBJECT * a0, X509_LOOKUP_TYPE a1, const X509_NAME * a2),
    (a0, a1, a2))
TWR_FUNCTION(int, X509_OBJECT_set1_X509, (X509_OBJECT * a0, X509 * a1),
    (a0, a1))
TWR_FUNCTION(int, X509_OBJECT_set1_X509_CRL, (X509_OBJECT * a0, X509_CRL * a1),
    (a0, a1))
TWR_VOID(X509_STORE_CTX_cleanup, (X509_STORE_CTX * a0),
    (a0))
TWR_VOID(X509_STORE_CTX_free, (X509_STORE_CTX * a0),
    (a0))
TWR_FUNCTION(int, X509_STORE_CTX_init, (X509_STORE_CTX * a0, X509_STORE * a1, X509 * a2, struct stack_st_X509 * a3),
    (a0, a1, a2, a3))
TWR_FUNCTION(X509_STORE_CTX *, X509_STORE_CTX_new, (void),
    ())
TWR_FUNCTION(int, X509_STORE_add_cert, (X509_STORE * a0, X509 * a1),
    (a0, a1))
TWR_FUNCTION(int, X509_STORE_add_crl, (X509_STORE * a0, X509_CRL * a1),
    (a0, a1))
TWR_FUNCTION(X509_LOOKUP *, X509_STORE_add_lookup, (X509_STORE * a0, X509_LOOKUP_METHOD * a1),
    (a0, a1))
TWR_VOID(X509_STORE_free, (X509_STORE * a0),
    (a0))
TWR_FUNCTION(struct stack_st_X509_OBJECT *, X509_STORE_get0_objects, (const X509_STORE * a0),
    (a0))
TWR_FUNCTION(int, X509_STORE_lock, (X509_STORE * a0),
    (a0))
TWR_FUNCTION(X509_STORE *, X509_STORE_new, (void),
    ())
TWR_FUNCTION(int, X509_STORE_set_default_paths, (X509_STORE * a0),
    (a0))
TWR_FUNCTION(int, X509_STORE_unlock, (X509_STORE * a0),
    (a0))
TWR_FUNCTION(int, X509_VERIFY_PARAM_clear_flags, (X509_VERIFY_PARAM * a0, unsigned long a1),
    (a0, a1))
TWR_FUNCTION(unsigned long, X509_VERIFY_PARAM_get_flags, (const X509_VERIFY_PARAM * a0),
    (a0))
TWR_FUNCTION(unsigned int, X509_VERIFY_PARAM_get_hostflags, (const X509_VERIFY_PARAM * a0),
    (a0))
TWR_FUNCTION(int, X509_VERIFY_PARAM_set1_host, (X509_VERIFY_PARAM * a0, const char * a1, size_t a2),
    (a0, a1, a2))
TWR_FUNCTION(int, X509_VERIFY_PARAM_set1_ip, (X509_VERIFY_PARAM * a0, const unsigned char * a1, size_t a2),
    (a0, a1, a2))
TWR_FUNCTION(int, X509_VERIFY_PARAM_set_flags, (X509_VERIFY_PARAM * a0, unsigned long a1),
    (a0, a1))
TWR_VOID(X509_VERIFY_PARAM_set_hostflags, (X509_VERIFY_PARAM * a0, unsigned int a1),
    (a0, a1))
TWR_FUNCTION(int, X509_add_ext, (X509 * a0, X509_EXTENSION * a1, int a2),
    (a0, a1, a2))
TWR_FUNCTION(int, X509_check_ca, (X509 * a0),
    (a0))
TWR_FUNCTION(int, X509_check_private_key, (const X509 * a0, const EVP_PKEY * a1),
    (a0, a1))
TWR_FUNCTION(int, X509_cmp, (const X509 * a0, const X509 * a1),
    (a0, a1))
TWR_VOID(X509_free, (X509 * a0),
    (a0))
TWR_VOID(X509_get0_signature, (const ASN1_BIT_STRING ** a0, const X509_ALGOR ** a1, const X509 * a2),
    (a0, a1, a2))
TWR_FUNCTION(const char *, X509_get_default_cert_dir, (void),
    ())
TWR_FUNCTION(const char *, X509_get_default_cert_dir_env, (void),
    ())
TWR_FUNCTION(const char *, X509_get_default_cert_file, (void),
    ())
TWR_FUNCTION(const char *, X509_get_default_cert_file_env, (void),
    ())
TWR_FUNCTION(void *, X509_get_ext_d2i, (const X509 * a0, int a1, int * a2, int * a3),
    (a0, a1, a2, a3))
TWR_FUNCTION(X509_NAME *, X509_get_issuer_name, (const X509 * a0),
    (a0))
TWR_FUNCTION(EVP_PKEY *, X509_get_pubkey, (X509 * a0),
    (a0))
TWR_FUNCTION(ASN1_INTEGER *, X509_get_serialNumber, (X509 * a0),
    (a0))
TWR_FUNCTION(X509_NAME *, X509_get_subject_name, (const X509 * a0),
    (a0))
TWR_FUNCTION(long, X509_get_version, (const X509 * a0),
    (a0))
TWR_FUNCTION(ASN1_TIME *, X509_getm_notAfter, (const X509 * a0),
    (a0))
TWR_FUNCTION(ASN1_TIME *, X509_getm_notBefore, (const X509 * a0),
    (a0))
TWR_FUNCTION(ASN1_TIME *, X509_gmtime_adj, (ASN1_TIME * a0, long a1),
    (a0, a1))
TWR_FUNCTION(X509 *, X509_new, (void),
    ())
TWR_FUNCTION(int, X509_set1_notAfter, (X509 * a0, const ASN1_TIME * a1),
    (a0, a1))
TWR_FUNCTION(int, X509_set1_notBefore, (X509 * a0, const ASN1_TIME * a1),
    (a0, a1))
TWR_FUNCTION(int, X509_set_issuer_name, (X509 * a0, const X509_NAME * a1),
    (a0, a1))
TWR_FUNCTION(int, X509_set_pubkey, (X509 * a0, EVP_PKEY * a1),
    (a0, a1))
TWR_FUNCTION(int, X509_set_subject_name, (X509 * a0, const X509_NAME * a1),
    (a0, a1))
TWR_FUNCTION(int, X509_set_version, (X509 * a0, long a1),
    (a0, a1))
TWR_FUNCTION(int, X509_sign, (X509 * a0, EVP_PKEY * a1, const EVP_MD * a2),
    (a0, a1, a2))
TWR_FUNCTION(unsigned long, X509_subject_name_hash, (X509 * a0),
    (a0))
TWR_FUNCTION(int, X509_up_ref, (X509 * a0),
    (a0))
TWR_FUNCTION(int, X509_verify_cert, (X509_STORE_CTX * a0),
    (a0))
TWR_FUNCTION(const char *, X509_verify_cert_error_string, (long a0),
    (a0))
TWR_FUNCTION(ASN1_TYPE *, d2i_ASN1_TYPE, (ASN1_TYPE ** a0, const unsigned char ** a1, long a2),
    (a0, a1, a2))
TWR_FUNCTION(X509 *, d2i_X509, (X509 ** a0, const unsigned char ** a1, long a2),
    (a0, a1, a2))
TWR_FUNCTION(X509_NAME *, d2i_X509_NAME, (X509_NAME ** a0, const unsigned char ** a1, long a2),
    (a0, a1, a2))
TWR_FUNCTION(int, i2d_X509, (const X509 * a0, unsigned char ** a1),
    (a0, a1))
TWR_FUNCTION(int, i2d_X509_CRL, (const X509_CRL * a0, unsigned char ** a1),
    (a0, a1))
TWR_FUNCTION(int, i2d_X509_NAME, (const X509_NAME * a0, unsigned char ** a1),
    (a0, a1))
