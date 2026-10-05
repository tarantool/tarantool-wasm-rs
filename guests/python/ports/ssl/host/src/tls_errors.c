#include <openssl/ssl.h>
#include <openssl/err.h>
#include <openssl/pem.h>
#include <openssl/x509v3.h>
#include <openssl/rand.h>

/* Preprocessor input only: the generated entries are converted to Rust data.
 * These are the same table/version choices as CPython 3.12's Modules/_ssl.c. */
#if OPENSSL_VERSION_NUMBER >= 0x30100000L
#include "ssl_error_data/_ssl_data_31.h"
#elif OPENSSL_VERSION_NUMBER >= 0x30000000L
#include "ssl_error_data/_ssl_data_300.h"
#else
#include "ssl_error_data/_ssl_data_111.h"
#endif
