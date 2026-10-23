local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')
local fiber = require('fiber')
local json = require('json')
local ffi = require('ffi')

ffi.cdef('int kill(int pid, int sig);')

local g = t.group('ssl_socket')

local function quote(value)
    return "'" .. value:gsub("'", "'\\''") .. "'"
end

local function read_file(path)
    local file = assert(fio.open(path, {'O_RDONLY'}))
    local data = file:read()
    file:close()

    return data
end

g.before_all(function(cg)
    cg.temp_dir = fio.tempdir()

    local state_path = fio.pathjoin(cg.temp_dir, 'server.json')
    local log_path = fio.pathjoin(cg.temp_dir, 'server.log')
    cg.pid_path = fio.pathjoin(cg.temp_dir, 'server.pid')

    local command = quote(os.getenv('PYTHON') or 'python3')
        .. ' '
        .. quote(
            fio.abspath('guests/python/tests/integration/ssl_socket_server.py')
        )
        .. ' '
        .. quote(state_path)
        .. ' '
        .. quote(fio.abspath('guests/python/tests/fixtures/ssl'))
        .. ' >'
        .. quote(log_path)
        .. ' 2>&1 & echo $! >'
        .. quote(cg.pid_path)
    assert(os.execute(command) == 0)

    local deadline = fiber.clock() + 10
    while not fio.path.exists(state_path) and fiber.clock() < deadline do
        fiber.sleep(0.01)
    end
    if not fio.path.exists(state_path) then
        error('TLS test server did not start: ' .. read_file(log_path))
    end
    cg.peer = json.decode(read_file(state_path))
end)

g.after_all(function(cg)
    if cg.pid_path ~= nil and fio.path.exists(cg.pid_path) then
        local pid = tonumber(read_file(cg.pid_path))
        if pid ~= nil then
            ffi.C.kill(pid, 15)
        end
    end
    if cg.temp_dir ~= nil then
        fio.rmtree(cg.temp_dir)
    end
end)

local function load_interpreter(cg, overrides)
    cg.guest_tmp = cg.guest_tmp or fio.tempdir()

    local config = {
        interpreter = true,
        inherit_network = true,
        allowed_ips = {'127.0.0.1'},
        allowed_ports = {
            cg.peer.tls_port,
            cg.peer.plain_port,
            cg.peer.coalesced_port,
            cg.peer.strict_port,
        },
        preopened_dirs = {
            {
                fio.abspath('guests/python/tests/fixtures/ssl'),
                '/certs',
                perms = 'ro',
            },
            {cg.guest_tmp, '/tmp', perms = 'rw'},
        },
    }

    for key, value in pairs(overrides or {}) do
        config[key] = value
    end
    cg.module =
        wasm.load(fio.abspath('guests/python/interpreter/adder.wasm'), config)

    t.assert_equals(
        wasm.interpreter_exec(
            cg.module,
            '__name__ = "ssl_socket_cases"\n'
                .. read_file(
                    'guests/python/tests/integration/ssl_socket_cases.py'
                )
        ),
        true
    )
end

g.before_each(load_interpreter)

g.test_standalone_python_component = function(cg)
    local url = 'https://localhost:' .. tostring(cg.peer.tls_port) .. '/'
    local function fetch(overrides, cafile)
        local config = {
            inherit_network = true,
            allowed_ips = {'127.0.0.1'},
            allowed_ports = {cg.peer.tls_port},
            preopened_dirs = {
                {
                    fio.abspath('guests/python/tests/fixtures/ssl'),
                    '/certs',
                    perms = 'ro',
                },
            },
        }

        for key, value in pairs(overrides) do
            config[key] = value
        end
        local component = wasm.load(
            fio.abspath(
                'guests/python/tests/components/https_smoke/client.wasm'
            ),
            config
        )

        local ok, result = pcall(wasm.call, component, 'fetch', url, cafile)

        wasm.drop(component)

        t.assert(ok, tostring(result))

        return result
    end

    t.assert_equals(fetch({}, '/certs/server.crt'), {true, 'wasi-https-ok'})

    local untrusted = fetch({}, nil)

    t.assert_equals(untrusted[1], false)
    t.assert_equals(
        untrusted[2],
        'SSLCertVerificationError: CERTIFICATE_VERIFY_FAILED'
    )

    for _, restrictions in ipairs({
        {inherit_network = false},
        {allowed_ips = {'192.0.2.1'}},
        {allowed_ports = {1}},
    }) do
        local rejected = fetch(restrictions, '/certs/server.crt')

        t.assert_equals(rejected[1], false)
        t.assert_str_contains(rejected[2], 'PermissionError:')
    end
end

g.test_private_socket_arguments = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'private_socket_arguments'),
        'private-socket-arguments'
    )
end

g.test_private_socket_exchange = function(cg)
    for _, tls12 in ipairs({false, true}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'private_socket_exchange',
                '/certs',
                cg.peer.tls_port,
                tls12
            ),
            'private-socket-exchange'
        )
    end
end

g.test_private_socket_nonblocking = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'private_socket_nonblocking',
            '/certs',
            cg.peer.tls_port
        ),
        'private-socket-nonblocking'
    )
end

g.test_private_socket_tracebacks = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'private_socket_tracebacks',
            '/certs',
            cg.peer.tls_port,
            cg.peer.plain_port
        ),
        'private-socket-tracebacks'
    )
end

g.test_private_socket_closed_transport = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'private_socket_closed_transport',
            '/certs',
            cg.peer.tls_port
        ),
        'private-socket-closed-transport'
    )
end

g.test_private_socket_shutdown = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'private_socket_shutdown',
            '/certs',
            cg.peer.coalesced_port
        ),
        'private-socket-shutdown'
    )
end

g.test_socket_empty_write_handshake = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'socket_empty_write_handshake',
            '/certs',
            cg.peer.tls_port
        ),
        'socket-empty-write-handshake'
    )
end

g.test_socket_buffer_rules = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'socket_buffer_rules',
            '/certs',
            cg.peer.tls_port
        ),
        'socket-buffer-rules'
    )
end

g.test_socket_pending_buffer_rules = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'socket_pending_buffer_rules',
            '/certs',
            cg.peer.tls_port
        ),
        'socket-pending-buffer-rules'
    )
end

g.test_socket_unconnected_buffer_rules = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'socket_unconnected_buffer_rules',
            '/certs',
            cg.peer.tls_port
        ),
        'socket-unconnected-buffer-rules'
    )
end

g.after_each(function(cg)
    if cg.module ~= nil then
        wasm.drop(cg.module)
    end

    if cg.client_pid_path ~= nil and fio.path.exists(cg.client_pid_path) then
        local pid = tonumber(read_file(cg.client_pid_path))
        if pid ~= nil then
            ffi.C.kill(pid, 15)
        end
        fio.unlink(cg.client_pid_path)
        cg.client_pid_path = nil
    end
    if cg.guest_tmp ~= nil then
        fio.rmtree(cg.guest_tmp)
        cg.guest_tmp = nil
    end
end)

local function free_port()
    local reserve = assert(require('socket')('AF_INET', 'SOCK_STREAM', 0))
    assert(reserve:bind('127.0.0.1', 0))
    local port = reserve:name().port
    reserve:close()

    return port
end

local function listener_case(
    cg,
    deferred,
    reject_first,
    sni,
    post_auth,
    private
)
    local port = free_port()

    wasm.drop(cg.module)
    cg.module = nil
    -- Accept checks the client's ephemeral source port through the same WASI
    -- ACL; these server tests explicitly allow ports while retaining IP ACL.
    load_interpreter(cg, {allowed_ports = {}})
    local state_path = fio.pathjoin(cg.temp_dir, 'client.json')
    local log_path = fio.pathjoin(cg.temp_dir, 'client.log')
    if fio.path.exists(state_path) then
        fio.unlink(state_path)
    end
    cg.client_pid_path = fio.pathjoin(cg.temp_dir, 'client.pid')

    local command = quote(os.getenv('PYTHON') or 'python3')
        .. ' '
        .. quote(
            fio.abspath('guests/python/tests/integration/ssl_socket_server.py')
        )
        .. ' --client '
        .. quote(state_path)
        .. ' '
        .. quote(fio.abspath('guests/python/tests/fixtures/ssl'))
        .. ' '
        .. tostring(port)
        .. ' '
        .. (
            post_auth and 'pha'
            or sni and reject_first and 'sni-alert-first'
            or reject_first and 'reject-first'
            or 'valid'
        )
        .. ' >'
        .. quote(log_path)
        .. ' 2>&1 & echo $! >'
        .. quote(cg.client_pid_path)
    assert(os.execute(command) == 0)
    if private then
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'private_socket_listener',
                '/certs',
                port,
                private == 'public-guard'
            ),
            'private-socket-listener'
        )
    else
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'listener_exchange',
                '/certs',
                port,
                deferred,
                reject_first,
                sni or false,
                post_auth or false
            ),
            'listener-exchange'
        )
    end
    local deadline = fiber.clock() + 8
    while not fio.path.exists(state_path) and fiber.clock() < deadline do
        fiber.sleep(0.01)
    end
    if not fio.path.exists(state_path) then
        error('TLS listener client did not finish: ' .. read_file(log_path))
    end

    t.assert_equals(
        json.decode(read_file(state_path)),
        {result = 'listener-client-ok'}
    )
end

g.test_private_socket_listener = function(cg)
    listener_case(cg, false, false, false, false, true)
end

g.test_socket_sni_reentry = function(cg)
    listener_case(cg, false, false, false, false, 'public-guard')
end

g.test_listener_accept = function(cg)
    listener_case(cg, false, false)
end

g.test_listener_live_sni = function(cg)
    for _, mode in ipairs({'live-enable', 'live-disable', 'live-replace'}) do
        listener_case(cg, true, false, mode)
    end
end

g.test_listener_post_handshake_auth = function(cg)
    listener_case(cg, false, false, false, true)
end

g.test_post_auth_socket_states = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'post_auth_socket_states',
            '/certs',
            cg.peer.tls_port
        ),
        'post-auth-socket-states'
    )
end

g.test_custom_socket_classes = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'custom_socket_classes',
            '/certs',
            cg.peer.tls_port
        ),
        'custom-socket-classes'
    )
end

g.test_socket_messages = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'socket_messages',
            '/certs',
            cg.peer.tls_port
        ),
        'socket-messages'
    )
end

g.test_socket_message_failures = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'socket_message_failures',
            '/certs',
            cg.peer.tls_port
        ),
        'socket-message-failures'
    )
end

g.test_socket_message_guards = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'socket_message_guards',
            '/certs',
            cg.peer.tls_port
        ),
        'socket-message-guards'
    )
end

g.test_socket_keylog = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'socket_keylog',
            '/certs',
            cg.peer.tls_port,
            '/tmp'
        ),
        'socket-keylog'
    )
end

g.test_socket_sendfile = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'socket_sendfile',
            '/certs',
            cg.peer.tls_port,
            '/tmp'
        ),
        'socket-sendfile'
    )
end

g.test_socket_ca_directory = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'socket_ca_directory',
            '/certs',
            cg.peer.tls_port,
            '/tmp'
        ),
        'socket-ca-directory'
    )
end

g.test_socket_live_trust = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'socket_live_trust',
            '/certs',
            cg.peer.tls_port,
            '/tmp'
        ),
        'socket-live-trust'
    )
end

g.test_socket_live_policy = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'socket_live_policy',
            '/certs',
            cg.peer.tls_port
        ),
        'socket-live-policy'
    )
end

g.test_socket_live_settings = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'socket_live_settings',
            '/certs',
            cg.peer.tls_port
        ),
        'socket-live-settings'
    )
end

g.test_socket_live_tls13 = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'socket_live_tls13',
            '/certs',
            cg.peer.tls_port
        ),
        'socket-live-tls13'
    )
end

g.test_socket_generic_roles = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'socket_generic_roles',
            '/certs',
            cg.peer.tls_port
        ),
        'socket-generic-roles'
    )
end

g.test_socket_live_alpn = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'socket_live_alpn',
            '/certs',
            cg.peer.tls_port
        ),
        'socket-live-alpn'
    )
end

g.test_socket_live_credentials = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'socket_live_credentials',
            '/certs',
            cg.peer.tls_port
        ),
        'socket-live-credentials'
    )
end

g.test_socket_live_key_exchange = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'socket_live_key_exchange',
            '/certs',
            cg.peer.tls_port
        ),
        'socket-live-key-exchange'
    )
end

g.test_socket_compression_and_npn = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'socket_compression_and_npn',
            '/certs',
            cg.peer.tls_port
        ),
        'socket-compression-and-npn'
    )
end

g.test_listener_deferred_handshake = function(cg)
    listener_case(cg, true, false)
end

g.test_listener_handshake_error_cleanup = function(cg)
    listener_case(cg, false, true)
end

g.test_listener_sni = function(cg)
    listener_case(cg, false, false, true)
    listener_case(cg, true, false, true)
end

g.test_listener_sni_alert_cleanup = function(cg)
    listener_case(cg, false, true, true)
end

g.test_listener_sni_pre_handshake_context = function(cg)
    for _, mode in ipairs({'pre-both', 'pre-enable', 'pre-disable'}) do
        listener_case(cg, true, false, mode)
    end
end

g.test_listener_nonblocking = function(cg)
    local port = free_port()

    wasm.drop(cg.module)
    cg.module = nil
    load_interpreter(cg, {allowed_ports = {}})

    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'nonblocking_listener',
            '/certs',
            port
        ),
        'nonblocking-listener'
    )
end

g.test_https_clients = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'https_clients',
            '/certs',
            cg.peer.tls_port
        ),
        'http.client-and-urllib'
    )
end

g.test_urllib3_clients = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'urllib3_clients',
            '/certs',
            cg.peer.strict_port
        ),
        'urllib3-https'
    )
end

g.test_urllib3_certificate_rejections = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'urllib3_certificate_rejections',
            '/certs',
            cg.peer.strict_port
        ),
        'urllib3-certificate-rejections'
    )
end

g.test_requests_clients = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'requests_clients',
            '/certs',
            cg.peer.strict_port
        ),
        'requests-https'
    )
end

g.test_requests_resources = function(cg)
    t.assert_equals(
        wasm.interpreter_call(cg.module, 'requests_resources'),
        'requests-resources'
    )
end

g.test_requests_default_verification = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'requests_default_verification',
            cg.peer.strict_port
        ),
        'requests-default-verification'
    )
end

g.test_requests_requires_guest_temp = function(cg)
    wasm.drop(cg.module)
    cg.module = nil
    load_interpreter(
        cg,
        {
            preopened_dirs = {
                {
                    fio.abspath('guests/python/tests/fixtures/ssl'),
                    '/certs',
                    perms = 'ro',
                },
            },
        }
    )

    t.assert_equals(
        wasm.interpreter_call(cg.module, 'requests_missing_temp'),
        'requests-needs-guest-temp'
    )
end

g.test_server_certificate_helpers = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'server_certificate_helpers',
            '/certs',
            cg.peer.tls_port
        ),
        'server-certificate-helpers'
    )
end

g.test_socket_session_resumption = function(cg)
    for _, tls12 in ipairs({false, true}) do
        for _, unconnected in ipairs({false, true}) do
            t.assert_equals(
                wasm.interpreter_call(
                    cg.module,
                    'socket_session_resumption',
                    '/certs',
                    cg.peer.tls_port,
                    tls12,
                    unconnected
                ),
                'socket-session-resumption'
            )
        end
    end
end

g.test_socket_connection_metadata = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'socket_connection_metadata',
            '/certs',
            cg.peer.tls_port
        ),
        'socket-connection-metadata'
    )
end

g.test_large_post = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'large_post',
            '/certs',
            cg.peer.tls_port
        ),
        2280000
    )
end

g.test_deferred_handshake_and_unwrap = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'deferred_handshake_and_unwrap',
            '/certs',
            cg.peer.tls_port
        ),
        'deferred-handshake-and-unwrap'
    )
end

g.test_nonblocking_and_timeout = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'nonblocking_and_timeout',
            '/certs',
            cg.peer.tls_port
        ),
        'nonblocking-and-timeout'
    )
end

g.test_nonblocking_exchange = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'nonblocking_exchange',
            '/certs',
            cg.peer.tls_port
        ),
        1700000
    )
end

g.test_switch_to_blocking_with_pending_write = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'switch_to_blocking_with_pending_write',
            '/certs',
            cg.peer.tls_port
        ),
        'read-with-pending-write'
    )
end

g.test_coalesced_unwrap = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'coalesced_unwrap',
            '/certs',
            cg.peer.coalesced_port
        ),
        'coalesced-close-and-plaintext'
    )
end

g.test_write_timeout = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'write_timeout',
            '/certs',
            cg.peer.tls_port
        ),
        'overall-write-timeout'
    )
end

g.test_read_completion_after_deadline = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'read_completion_after_deadline',
            '/certs',
            cg.peer.tls_port
        ),
        'late-read-preserved'
    )
end

g.test_asyncio_tls_client = function(cg)
    wasm.drop(cg.module)
    -- asyncio's self-pipe uses an ephemeral loopback TCP pair on WASI.
    -- Permit those ports while retaining the loopback-only IP restriction.
    load_interpreter(cg, {allowed_ports = {}})

    for _, tls12 in ipairs({false, true}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'asyncio_tls_client',
                '/certs',
                cg.peer.tls_port,
                tls12
            ),
            'asyncio-tls-client'
        )
    end
end

g.test_asyncio_tls_hostname = function(cg)
    wasm.drop(cg.module)
    load_interpreter(cg, {allowed_ports = {}, allow_ip_name_lookup = true})

    for _, tls12 in ipairs({false, true}) do
        t.assert_equals(
            wasm.interpreter_call(
                cg.module,
                'asyncio_tls_client',
                '/certs',
                cg.peer.tls_port,
                tls12,
                'localhost'
            ),
            'asyncio-tls-client'
        )
    end
end

g.test_asyncio_dns_denied = function(cg)
    wasm.drop(cg.module)
    load_interpreter(cg, {allowed_ports = {}, allow_ip_name_lookup = false})

    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'asyncio_dns_denied',
            cg.peer.tls_port
        ),
        'asyncio-dns-denied'
    )
end

g.test_eof_and_plain_peer = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'eof_and_plain_peer',
            '/certs',
            cg.peer.tls_port,
            cg.peer.plain_port
        ),
        'eof-and-plain-peer'
    )
end

g.test_certificate_rejections = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'certificate_rejections',
            '/certs',
            cg.peer.tls_port
        ),
        'socket-certificate-rejections'
    )
end

g.test_unsupported_io = function(cg)
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'unsupported_io',
            '/certs',
            cg.peer.tls_port
        ),
        'unsupported-io-rejected'
    )
end

g.test_network_disabled = function(cg)
    wasm.drop(cg.module)
    cg.module = nil
    load_interpreter(cg, {inherit_network = false})

    t.assert_equals(
        wasm.interpreter_call(cg.module, 'denied_connect', cg.peer.tls_port),
        'network-denied'
    )
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'urllib3_denied_connect',
            '/certs',
            cg.peer.strict_port
        ),
        'urllib3-network-denied'
    )
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'requests_denied_connect',
            '/certs',
            cg.peer.strict_port
        ),
        'requests-network-denied'
    )
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'denied_listener',
            cg.peer.strict_port
        ),
        'listener-network-denied'
    )
end

g.test_port_acl = function(cg)
    wasm.drop(cg.module)
    cg.module = nil
    load_interpreter(cg, {allowed_ports = {cg.peer.plain_port}})

    t.assert_equals(
        wasm.interpreter_call(cg.module, 'denied_connect', cg.peer.tls_port),
        'network-denied'
    )
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'urllib3_denied_connect',
            '/certs',
            cg.peer.strict_port
        ),
        'urllib3-network-denied'
    )
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'requests_denied_connect',
            '/certs',
            cg.peer.strict_port
        ),
        'requests-network-denied'
    )
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'denied_listener',
            cg.peer.strict_port
        ),
        'listener-network-denied'
    )
end

g.test_ip_acl = function(cg)
    wasm.drop(cg.module)
    cg.module = nil
    load_interpreter(cg, {allowed_ips = {'192.0.2.1'}})

    t.assert_equals(
        wasm.interpreter_call(cg.module, 'denied_connect', cg.peer.tls_port),
        'network-denied'
    )
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'urllib3_denied_connect',
            '/certs',
            cg.peer.strict_port
        ),
        'urllib3-network-denied'
    )
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'requests_denied_connect',
            '/certs',
            cg.peer.strict_port
        ),
        'requests-network-denied'
    )
    t.assert_equals(
        wasm.interpreter_call(
            cg.module,
            'denied_listener',
            cg.peer.strict_port
        ),
        'listener-network-denied'
    )
end
