local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')

local g = t.group('ssl_bio')

g.before_each(function(cg)
    cg.guest_tmp = fio.tempdir()
    cg.module = wasm.load(fio.abspath('guests/python/interpreter/adder.wasm'), {
        interpreter = true,
        inherit_network = false,
        preopened_dirs = {
            {
                fio.abspath('guests/python/tests/fixtures/ssl'),
                '/certs',
                perms = 'ro',
            },
            {cg.guest_tmp, '/tmp', perms = 'rw'},
        },
    })

    local file = assert(
        fio.open(
            'guests/python/tests/integration/ssl_bio_cases.py',
            {'O_RDONLY'}
        )
    )
    local script = file:read()
    file:close()

    -- Avoid running the native command-line entrypoint inside the interpreter.
    t.assert_equals(
        wasm.interpreter_exec(
            cg.module,
            '__name__ = "ssl_bio_cases"\n' .. script
        ),
        true
    )
end)

g.after_each(function(cg)
    if cg.module ~= nil then
        wasm.drop(cg.module)
    end

    if cg.guest_tmp ~= nil then
        fio.rmtree(cg.guest_tmp)
    end
end)

g.test_default_trust_overrides = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'default_trust_overrides',
            '/certs',
            '/tmp'
        ),
        'default-trust-overrides'
    )
end

g.test_defaults_and_memory_bio = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'defaults_and_memory_bio'),
        'defaults-and-bio'
    )
end

g.test_memory_bio_buffer_rules = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'memory_bio_buffer_rules'),
        'memory-bio-buffer-rules'
    )
end

g.test_tls_buffer_rules = function(cg)
    for _, tls12 in ipairs({false, true}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'tls_buffer_rules',
                '/certs',
                tls12
            ),
            'tls-buffer-rules'
        )
    end
end

g.test_hostname_argument_rules = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'hostname_argument_rules'),
        'hostname-argument-rules'
    )
end

g.test_tls13_and_tls12_exchange = function(cg)
    for _, tls12 in ipairs({false, true}) do
        local result =
            wasm.interpreter_call(cg.module, 'exchange', '/certs', tls12)

        t.assert_equals(result[1], tls12 and 'TLSv1.2' or 'TLSv1.3')
        t.assert_equals(result[2], 'http/1.1')
        t.assert_gt(result[3], 0)
    end
end

g.test_certificate_rejections = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'rejection',
            '/certs',
            'wrong.example',
            true
        ),
        'certificate-rejected'
    )
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'rejection',
            '/certs',
            'localhost',
            false
        ),
        'certificate-rejected'
    )
end

g.test_shutdown_and_eof = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'shutdown_and_eof', '/certs'),
        'clean-close-and-eof'
    )
end

g.test_large_buffered_input = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'large_buffered_input', '/certs'),
        2160000
    )
end

g.test_certificate_metadata = function(cg)
    for _, verify in ipairs({0, 1, 2}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'certificate_metadata',
                '/certs',
                verify,
                false
            ),
            'certificate-metadata'
        )
    end

    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'certificate_metadata',
            '/certs',
            2,
            true
        ),
        'certificate-metadata'
    )
end

g.test_certificate_helpers = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'certificate_helpers', '/certs'),
        'certificate-helpers'
    )
end

g.test_private_module_constants = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'private_module_constants'),
        'private-module-constants'
    )
end

g.test_private_certificate_helpers = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'private_certificate_helpers',
            '/certs'
        ),
        'private-certificate-helpers'
    )
end

g.test_private_certificate_chains = function(cg)
    for _, mutual in ipairs({false, true}) do
        for _, verified in ipairs({false, true}) do
            t.assert_equals(
                wasm.interpreter_call(
                    cg.module,
                    'private_certificate_chains',
                    '/certs',
                    mutual,
                    verified
                ),
                'private-certificate-chains'
            )
        end
    end
end

g.test_private_certificate_cleanup = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'private_certificate_cleanup'),
        'private-certificate-cleanup'
    )
end

g.test_private_hostname_flags = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'private_hostname_flags', '/certs'),
        'private-hostname-flags'
    )
end

g.test_private_binary_alpn = function(cg)
    for _, tls12 in ipairs({false, true}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'private_binary_alpn',
                '/certs',
                tls12
            ),
            'private-binary-alpn'
        )
    end
end

g.test_private_bio_arguments = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'private_bio_arguments'),
        'private-bio-arguments'
    )
end

g.test_private_bio_dead_owner = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'private_bio_dead_owner', '/certs'),
        'private-bio-dead-owner'
    )
end

g.test_private_bio_exchange = function(cg)
    for _, tls12 in ipairs({false, true}) do
        for _, supplied in ipairs({false, true}) do
            t.assert_equals(
                wasm.interpreter_call(
                    cg.module,
                    'private_bio_exchange',
                    '/certs',
                    tls12,
                    supplied
                ),
                'private-bio-exchange'
            )
        end
    end
end

g.test_credential_formats = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'credential_formats', '/certs'),
        'der-and-encrypted-key'
    )
end

g.test_credential_load_errors = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'credential_load_errors', '/certs'),
        'credential-load-errors'
    )
end

g.test_live_credentials = function(cg)
    for _, tls12 in ipairs({false, true}) do
        for _, role in ipairs({'client', 'server'}) do
            t.assert_equals(
                wasm.interpreter_call(
                    cg.module,
                    'live_credentials',
                    '/certs',
                    role,
                    tls12
                ),
                'live-credentials'
            )
        end
    end
end

g.test_credential_security_history = function(cg)
    for _, cached in ipairs({false, true}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'credential_security_history',
                '/certs',
                cached
            ),
            'credential-security-history'
        )
    end
end

g.test_credential_algorithms = function(cg)
    for _, cached in ipairs({false, true}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'credential_algorithms',
                '/certs',
                cached
            ),
            'credential-algorithms'
        )
    end
end

g.test_policy_settings = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'policy_settings', '/certs'),
        'policy-settings'
    )
end

g.test_protocol_bounds = function(cg)
    for _, cached in ipairs({false, true}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'protocol_bounds',
                '/certs',
                cached
            ),
            'protocol-bounds'
        )
    end
end

g.test_module_constants_and_star_import = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'module_constants_and_star_import'),
        'module-constants-and-star-import'
    )
end

g.test_symbolic_errors = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'symbolic_errors', '/certs'),
        'symbolic-errors'
    )
end

g.test_unexpected_eof_options = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'unexpected_eof_options', '/certs'),
        'unexpected-eof-options'
    )
end

g.test_verification_messages = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'verification_messages', '/certs'),
        'verification-messages'
    )
end

g.test_raw_policy_masks = function(cg)
    for _, cached in ipairs({false, true}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'raw_policy_masks',
                '/certs',
                cached
            ),
            'raw-policy-masks'
        )
    end
end

g.test_policy_mask_normalization = function(cg)
    for _, cached in ipairs({false, true}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'policy_mask_normalization',
                '/certs',
                cached
            ),
            'policy-mask-normalization'
        )
    end
end

g.test_legacy_policy_rejection = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'legacy_policy_rejection', '/certs'),
        'legacy-policy-rejected'
    )
end

g.test_legacy_protocol_exchange = function(cg)
    for _, cached in ipairs({false, true}) do
        for _, fixed in ipairs({false, true}) do
            for _, tls11 in ipairs({false, true}) do
                t.assert_equals(
                    wasm.interpreter_call(
                        cg.module,
                        'legacy_protocol_exchange',
                        '/certs',
                        tls11,
                        fixed,
                        cached
                    ),
                    'legacy-protocol-exchange'
                )
            end
        end
    end
end

g.test_policy_exchange = function(cg)
    for _, mode in ipairs({
        'cipher',
        'legacy-cipher-metadata',
        'disable-tls13',
        'common-name-enabled',
        'generic-tls',
        'fixed-tls12',
    }) do
        t.assert_equals(
            wasm.interpreter_call(cg.module, 'policy_exchange', '/certs', mode),
            'policy-applied'
        )
    end
    for _, mode in ipairs({
        'disable-tls12',
        'common-name-disabled',
        'crl-required',
    }) do
        t.assert_equals(
            wasm.interpreter_call(cg.module, 'policy_exchange', '/certs', mode),
            'policy-rejected'
        )
    end
end

g.test_store_introspection = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'store_introspection', '/certs'),
        'store-introspection'
    )
end

g.test_store_pem_whitespace = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'store_pem_whitespace', '/certs'),
        'store-pem-whitespace'
    )
end

g.test_connection_metadata = function(cg)
    for _, tls12 in ipairs({false, true}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'connection_metadata',
                '/certs',
                tls12
            ),
            'connection-metadata'
        )
    end
end

g.test_session_resumption = function(cg)
    for _, tls12 in ipairs({false, true}) do
        for _, setter in ipairs({false, true}) do
            t.assert_equals(
                wasm.interpreter_call(
                    cg.module,
                    'session_resumption',
                    '/certs',
                    tls12,
                    setter
                ),
                'session-resumption'
            )
        end
    end
end

g.test_session_errors = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'session_errors', '/certs'),
        'session-errors'
    )
end

g.test_key_exchange_settings = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'key_exchange_settings', '/certs'),
        'key-exchange-settings'
    )
end

g.test_public_pem_password_rejection = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'public_pem_password_rejection',
            '/certs'
        ),
        'public-pem-password-rejected'
    )
end

g.test_key_exchange = function(cg)
    for _, mode in ipairs({
        'dh',
        'dh-missing',
        'ecdh12',
        'ecdh13',
        'mismatch12',
        'mismatch13',
    }) do
        local expected = (mode == 'dh-missing' or mode:match('^mismatch'))
                and 'key-exchange-rejected'
            or 'key-exchange-ok'

        t.assert_equals(
            wasm.interpreter_call(cg.module, 'key_exchange', '/certs', mode),
            expected
        )
    end
end

g.test_sni_exchange = function(cg)
    for _, tls12 in ipairs({false, true}) do
        for _, mode in ipairs({
            'switch',
            'idna',
            'legacy',
            'absent',
            'ip',
            'alert',
        }) do
            t.assert_equals(
                wasm.interpreter_call(
                    cg.module,
                    'sni_exchange',
                    '/certs',
                    mode,
                    tls12
                ),
                mode == 'alert' and 'sni-rejected' or 'sni-exchange'
            )
        end
    end
end

g.test_sni_settings = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'sni_settings'),
        'sni-settings'
    )
end

g.test_sni_pre_handshake_context = function(cg)
    for _, tls12 in ipairs({false, true}) do
        for _, mode in ipairs({'pre-both', 'pre-enable', 'pre-disable'}) do
            t.assert_equals(
                wasm.interpreter_call(
                    cg.module,
                    'sni_exchange',
                    '/certs',
                    mode,
                    tls12
                ),
                'sni-exchange'
            )
        end
    end
end

g.test_sni_hello_retry_context = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module, 'sni_exchange', '/certs', 'hrr', false
        ),
        'sni-exchange'
    )
end

g.test_sni_acknowledgement = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'sni_acknowledgement', '/certs'),
        'sni-acknowledgement'
    )
end

g.test_tls13_settings = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'tls13_settings', '/certs'),
        'tls13-settings'
    )
end

g.test_custom_bio_classes = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'custom_bio_classes', '/certs'),
        'custom-bio-classes'
    )
end

g.test_module_metadata = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'module_metadata'),
        'module-metadata'
    )
end

g.test_random_helpers = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'random_helpers'),
        'random-helpers'
    )
end

g.test_default_verify_paths = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'default_verify_paths', '/certs'),
        'default-verify-paths'
    )
end

g.test_purpose_factories = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'purpose_factories', '/certs'),
        'purpose-factories'
    )
end

g.test_compression_and_npn = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'compression_and_npn', '/certs'),
        'compression-and-npn'
    )
end

g.test_ca_directory_settings = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'ca_directory_settings'),
        'ca-directory-settings'
    )
end

g.test_live_trust_store = function(cg)
    for _, mode in ipairs({
        'cafile',
        'cadata',
        'directory-change',
        'directory-new',
        'crl',
        'mutual',
    }) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'live_trust_store',
                '/certs',
                '/tmp',
                mode
            ),
            'live-trust-store'
        )
    end
end

g.test_live_context_policy = function(cg)
    for _, field in ipairs({
        'options',
        'verify-flags',
        'common-name',
        'ciphers',
    }) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'live_context_policy',
                '/certs',
                field
            ),
            'live-context-policy'
        )
    end
end

g.test_live_connection_settings = function(cg)
    for _, field in ipairs({
        'maximum',
        'minimum',
        'inverted',
        'verify-on',
        'verify-off',
        'check-on',
    }) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'live_connection_settings',
                '/certs',
                field
            ),
            'live-connection-settings'
        )
    end
end

g.test_live_tls13_settings = function(cg)
    for _, mode in ipairs({'tickets', 'client-pha', 'server-pha'}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'live_tls13_settings',
                '/certs',
                mode
            ),
            'live-tls13-settings'
        )
    end
end

g.test_generic_context_roles = function(cg)
    for _, settings in ipairs({{false, false}, {true, false}, {
        false,
        true,
    }}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'generic_context_roles',
                '/certs',
                settings[1],
                settings[2]
            ),
            'generic-context-roles'
        )
    end
end

g.test_live_sni = function(cg)
    for _, tls12 in ipairs({false, true}) do
        for _, mode in ipairs({
            'enable',
            'disable',
            'replace',
            'invalid',
            'fallback-enable',
            'fallback-disable',
            'current-enable',
            'current-disable',
            'self-disable',
            'self-alert',
        }) do
            t.assert_equals(
                wasm.interpreter_call(
                    cg.module,
                    'live_sni',
                    '/certs',
                    mode,
                    tls12
                ),
                'live-sni'
            )
        end
    end
end

g.test_live_alpn = function(cg)
    for _, tls12 in ipairs({false, true}) do
        for _, mode in ipairs({
            'client',
            'server',
            'enable',
            'disable',
            'no-overlap',
        }) do
            t.assert_equals(
                wasm.interpreter_call(
                    cg.module,
                    'live_alpn',
                    '/certs',
                    mode,
                    tls12
                ),
                'live-alpn'
            )
        end
    end
end

g.test_live_key_exchange = function(cg)
    for _, mode in ipairs({'dh', 'curve'}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'live_key_exchange',
                '/certs',
                mode
            ),
            'live-key-exchange'
        )
    end
end

g.test_dh_security_history = function(cg)
    for _, cached in ipairs({false, true}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'dh_security_history',
                '/certs',
                cached
            ),
            'dh-security-history'
        )
    end
end

g.test_security_level_inheritance = function(cg)
    for _, cached in ipairs({false, true}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'security_level_inheritance',
                '/certs',
                cached
            ),
            'security-level-inheritance'
        )
    end
end

g.test_suiteb_policy_inheritance = function(cg)
    for _, cached in ipairs({false, true}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'suiteb_policy_inheritance',
                '/certs',
                cached
            ),
            'suiteb-policy-inheritance'
        )
    end
end

g.test_ca_directory_guest_permissions = function(cg)
    local directory = fio.pathjoin(cg.guest_tmp, 'host-certs')
    assert(fio.mkdir(directory))
    assert(
        fio.copyfile(
            'guests/python/tests/fixtures/ssl/server.crt',
            fio.pathjoin(directory, 'ce275665.0')
        )
    )
    local script = [[
import ssl
def directory_permissions(path, allowed):
    client = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    client.load_verify_locations(capath=path)
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server.load_cert_chain('/certs/server.crt', '/certs/server.key')
    ci, co, si, so = [ssl.MemoryBIO() for _ in range(4)]
    pair = (
        client.wrap_bio(ci, co, server_hostname='localhost'),
        server.wrap_bio(si, so, server_side=True),
        ci, co, si, so,
    )
    try:
        try:
            handshake(pair)
        except ssl.SSLCertVerificationError:
            assert not allowed
        else:
            assert allowed
        assert client.cert_store_stats()['x509'] == int(allowed)
    finally:
        dispose(pair)
directory_permissions(%s, False)
directory_permissions('/tmp/host-certs', True)
]]
    t.assert_equals(
        wasm.interpreter_exec(
            cg.module,
            script:format(string.format('%q', directory))
        ),
        true
    )
end

g.test_ca_directory_guest_limits = function(cg)
    t.assert_equals(
        wasm.interpreter_exec(
            cg.module,
            [[
import os, ssl
context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
context.load_verify_locations(
    capath=':'.join('/absent/' + str(i) for i in range(16))
)
try:
    context.load_verify_locations(capath='/absent/17')
except ValueError:
    pass
else:
    raise AssertionError('unbounded directory registration')
os.mkdir('/tmp/large-capath')
with open('/tmp/large-capath/ce275665.0', 'wb') as file:
    file.write(b'x' * (1024 * 1024 + 1))
context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
context.load_verify_locations(capath='/tmp/large-capath')
incoming, outgoing = ssl.MemoryBIO(), ssl.MemoryBIO()
try:
    context.wrap_bio(incoming, outgoing, server_hostname='localhost')
except ValueError:
    pass
else:
    raise AssertionError('unbounded directory snapshot')
assert incoming._owner is outgoing._owner is None
]]
        ),
        true
    )
end

g.test_ca_directory = function(cg)
    for _, mode in ipairs({
        'good',
        'pathlike',
        'bytes',
        'multiple',
        'gap',
        'wrong-hash',
        'plain-name',
        'malformed',
        'collision',
        'missing',
        'mixed',
        'crl-good',
        'crl-revoked',
        'crl-gap',
        'aux-rejected',
        'mutual',
    }) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'ca_directory',
                '/certs',
                '/tmp',
                mode
            ),
            'ca-directory'
        )
    end
end

g.test_empty_write_handshake = function(cg)
    for _, tls12 in ipairs({false, true}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'empty_write_handshake',
                '/certs',
                tls12
            ),
            'empty-write-handshake'
        )
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'empty_write_handshake',
                '/certs',
                tls12,
                false
            ),
            'empty-write-rejected'
        )
    end
end

g.test_message_exchange = function(cg)
    for _, tls12 in ipairs({false, true}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'message_exchange',
                '/certs',
                tls12
            ),
            'message-exchange'
        )
    end
end

g.test_message_settings = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'message_settings', '/certs'),
        'message-settings'
    )
end

g.test_message_failures = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'message_failures', '/certs'),
        'message-failures'
    )
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'message_failures', '/certs', true),
        'message-failures'
    )
end

g.test_message_sni_switch = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'message_sni_switch', '/certs'),
        'message-sni-switch'
    )
end

g.test_message_queue_failures = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'message_queue_failures', '/certs'),
        'message-queue-failures'
    )
end

g.test_keylog_settings = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'keylog_settings', '/certs', '/tmp'),
        'keylog-settings'
    )
end

g.test_keylog_failures = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'keylog_failures', '/certs'),
        'keylog-failures'
    )
end

g.test_keylog_exchange = function(cg)
    for _, tls12 in ipairs({false, true}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'keylog_exchange',
                '/certs',
                '/tmp',
                tls12
            ),
            'keylog-exchange'
        )
    end

    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'keylog_exchange',
            '/certs',
            '/tmp',
            false,
            true
        ),
        'keylog-exchange'
    )
end

g.test_keylog_guest_permissions = function(cg)
    t.assert_equals(
        wasm.interpreter_exec(
            cg.module,
            [[
import ssl
context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
for path in ('/certs/keylog-forbidden.log', '/not-preopened/keylog.log'):
    try:
        context.keylog_filename = path
    except OSError:
        pass
    else:
        raise AssertionError('key log opened without guest write capability')
    assert context.keylog_filename is None
]]
        ),
        true
    )
    t.assert_not(
        fio.path.exists('guests/python/tests/fixtures/ssl/keylog-forbidden.log')
    )
end

g.test_post_handshake_auth = function(cg)
    for _, mode in ipairs({
        'cert',
        'optional',
        'missing-required',
        'untrusted',
        'disabled',
        'no-verification',
    }) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'post_handshake_auth',
                '/certs',
                mode,
                false
            ),
            (mode == 'cert' or mode == 'optional') and 'post-authenticated'
                or 'post-auth-rejected'
        )
    end

    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'post_handshake_auth',
            '/certs',
            'optional',
            true
        ),
        'post-auth-rejected'
    )
end

g.test_sni_callback_failures = function(cg)
    for _, tls12 in ipairs({false, true}) do
        for _, mode in ipairs({'exception', 'bad-return', 'switch-alert'}) do
            t.assert_equals(
                wasm.interpreter_call(
                    cg.module,
                    'sni_exchange',
                    '/certs',
                    mode,
                    tls12
                ),
                'sni-rejected'
            )
        end
    end
end

g.test_crl_and_aux_verification = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'store_exchange', '/certs', 'valid'),
        'store-accepted'
    )

    for _, mode in ipairs({'missing', 'revoked', 'aux-rejected'}) do
        t.assert_equals(
            wasm.interpreter_call(cg.module, 'store_exchange', '/certs', mode),
            'store-rejected'
        )
    end
end
