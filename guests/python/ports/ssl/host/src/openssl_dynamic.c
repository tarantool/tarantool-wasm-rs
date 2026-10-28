/* Keep openssl-rs' typed API while resolving its native calls at runtime. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <openssl/ssl.h>
#include <openssl/x509v3.h>
#include <openssl/err.h>
#include <openssl/pem.h>
#include <openssl/rand.h>
#include <openssl/comp.h>

#if OPENSSL_VERSION_MAJOR != 3
#error "The optional Python SSL backend requires OpenSSL 3 build headers"
#endif

struct functions {
#define TWR_FUNCTION(result, name, params, args) __typeof__(name) *name;
#define TWR_VOID(name, params, args) __typeof__(name) *name;
#include "openssl_functions.h"
#undef TWR_FUNCTION
#undef TWR_VOID
};

static struct functions functions;
static pthread_once_t once = PTHREAD_ONCE_INIT;
static char unavailable[512];

static void initialize(void)
{
    const char *path = getenv("TWR_OPENSSL_LIBRARY");
    void *library;
    struct functions loaded;
    int flags = RTLD_NOW | RTLD_LOCAL;

    if (path == NULL) {
#ifdef __APPLE__
        path = "/opt/homebrew/opt/openssl@3/lib/libssl.3.dylib";
#else
        path = "libssl.so.3";
#endif
    }
#ifdef RTLD_DEEPBIND
    /* Tarantool may export another OpenSSL. Internal library calls must use
     * this library's dependencies rather than that process-global ABI. */
    flags |= RTLD_DEEPBIND;
#endif
    library = dlopen(path, flags);
    if (library == NULL) {
        snprintf(unavailable, sizeof(unavailable),
                 "Python SSL unavailable: cannot load OpenSSL 3 (%s): %s",
                 path, dlerror());
        return;
    }

    __typeof__(OpenSSL_version_num) *version =
        (__typeof__(version))dlsym(library, "OpenSSL_version_num");
    if (version == NULL || (version() >> 28) != 3) {
        snprintf(unavailable, sizeof(unavailable),
                 "Python SSL unavailable: incompatible OpenSSL version");
        dlclose(library);
        return;
    }

    /* Resolve through this handle, including its matching libcrypto dependency.
     * Never use symbols from the Tarantool executable or RTLD_DEFAULT. */
#define TWR_FUNCTION(result, name, params, args)                            \
    loaded.name = (__typeof__(loaded.name))dlsym(library, #name);           \
    if (loaded.name == NULL) {                                             \
        snprintf(unavailable, sizeof(unavailable),                         \
                 "Python SSL unavailable: OpenSSL 3 is missing %s", #name); \
        dlclose(library);                                                  \
        return;                                                           \
    }
#define TWR_VOID(name, params, args) TWR_FUNCTION(void, name, params, args)
#include "openssl_functions.h"
#undef TWR_FUNCTION
#undef TWR_VOID

    /* Publish only a complete table. Retain the library for native objects,
     * callbacks and openssl-rs' process-global initialization state. */
    functions = loaded;
}

const char *twr_python_ssl_error(void)
{
    pthread_once(&once, initialize);
    return unavailable[0] == '\0' ? NULL : unavailable;
}

static void require_backend(void)
{
    /* WIT entry points return a typed error before reaching these wrappers.
     * This also initializes direct openssl-rs calls in native unit tests. */
    if (twr_python_ssl_error() != NULL)
        abort();
}

#define TWR_FUNCTION(result, name, params, args) \
    result name params                         \
    {                                          \
        require_backend();                     \
        return functions.name args;            \
    }
#define TWR_VOID(name, params, args) \
    void name params                \
    {                               \
        require_backend();          \
        functions.name args;        \
    }
#include "openssl_functions.h"
#undef TWR_FUNCTION
#undef TWR_VOID

void ERR_set_error(int library, int reason, const char *format, ...)
{
    va_list args;

    require_backend();
    va_start(args, format);
    functions.ERR_vset_error(library, reason, format, args);
    va_end(args);
}
