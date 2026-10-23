local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')
local socket = require('socket')
local fiber = require('fiber')
local json = require('json')

local g = t.group('go_capabilities')
local observations = {
    external_https = {
        status = 'skipped',
        reason = 'No external service in reproducible tests.',
    },
    tls = {
        status = 'known-limitation',
        reason = 'Adapter explicitly rejects TLS sockets.',
    },
}

g.after_all(function()
    fio.mktree('guests/go/dist')
    local file =
        assert(io.open('guests/go/dist/capabilities-observed.json', 'w'))
    file:write(json.encode(observations))
    file:close()
end)

local function invoke(name, options, direct)
    local path = direct and 'direct.wasm' or 'sdk.wasm'
    local m =
        wasm.load('guests/go/tests/components/probes/' .. path, options or {})

    local ok, value = pcall(wasm.call, m, 'check', name)

    wasm.drop(m)

    t.assert(ok, tostring(value))

    return value
end

local function with_server(handler, test)
    -- Drain queued Wasm completion wakeups before Lua's coio DNS in tcp_server.
    fiber.sleep(0)
    local server = assert(socket.tcp_server('127.0.0.1', 0, function(client)
        -- tcp_server owns and closes each accepted client after this returns.
        handler(client)
    end))

    local ok, err =
        pcall(test, '127.0.0.1:' .. server:name().port, server:name().port)
    server:close()
    if not ok then
        error(err, 0)
    end
end

g.test_compression = function()
    t.assert_equals(invoke('compression'), 'ok')
    observations.compression = {
        status = 'works',
        scenario = 'Compression roundtrips and checksum vectors.',
    }
end

g.test_explicit_enable = function()
    t.assert_str_contains(invoke('unconfigured'), 'Netdev not set')
end

g.test_dns = function()
    local value = invoke('dns')

    t.assert(
        value:find('127.0.0.1', 1, true) or value:find('::1', 1, true),
        value
    )
    observations.dns = {
        status = 'works',
        compiled = true,
        runtime = value,
        endpoint = 'localhost',
    }

    -- Wasmtime maps a disabled resolver to this WIT error code.
    t.assert_str_contains(
        invoke('dns', {allow_ip_name_lookup = false}),
        'permanent-resolver-failure'
    )
end

g.test_tcp = function()
    with_server(function(client)
        t.assert_equals(client:read(5, 2), 'hello')
        client:write('he')
        fiber.sleep(0.01)
        client:write('llo')
    end, function(endpoint)
        t.assert_equals(invoke('tcp|' .. endpoint), 'hello')
    end)
    observations.tcp = {
        status = 'works',
        compiled = true,
        endpoint = 'local-loopback',
        runtime = 'hello',
    }
end

g.test_http = function()
    with_server(function(client)
        local request = client:read({delimiter = '\r\n\r\n', chunk = 4096}, 2)

        t.assert_str_contains(request, 'GET / HTTP/1.1')
        client:write(
            'HTTP/1.1 200 OK\r\nContent-Length: 4\r\n'
                .. 'Connection: close\r\n\r\npo'
        )
        fiber.sleep(0.01)
        client:write('ng')
    end, function(endpoint)
        t.assert_equals(invoke('http|' .. endpoint), 'pong')
    end)
    observations.http = {
        status = 'works',
        compiled = true,
        endpoint = 'local-loopback',
        runtime = 'pong',
    }
end

g.test_network_permissions = function()
    with_server(function(client)
        client:read(1, 1)
    end, function(endpoint, port)
        for _, options in ipairs({
            {inherit_network = false},
            {allow_tcp = false},
            {allowed_ports = {port == 65535 and 65534 or port + 1}},
            {allowed_ips = {'192.0.2.1'}},
        }) do
            t.assert_str_contains(
                invoke('tcp|' .. endpoint, options),
                'access-denied'
            )
        end

        t.assert_str_contains(invoke('tls|' .. endpoint), 'not supported')
    end)
    observations.network_policy = {
        status = 'works',
        scenario = 'DNS, TCP, IP and port restrictions enforced by host.',
    }
end

g.test_read_timeout_and_close = function()
    with_server(function(client)
        client:read(1, 1)
    end, function(endpoint)
        t.assert_equals(invoke('tcp-timeout|' .. endpoint), 'timeout')
        t.assert_equals(invoke('tcp-close|' .. endpoint), 'closed')
    end)
end

g.test_eof_and_resource_reuse = function()
    with_server(function(client)
        client:write('done')
    end, function(endpoint)
        t.assert_equals(invoke('tcp-reuse|' .. endpoint), 'done')
    end)
end

g.test_direct_build = function()
    local value = invoke('dns', nil, true)

    t.assert(
        value:find('127.0.0.1', 1, true) or value:find('::1', 1, true),
        value
    )
    with_server(function(client)
        t.assert_equals(client:read(5, 2), 'hello')
        client:write('hello')
    end, function(endpoint)
        t.assert_equals(invoke('tcp|' .. endpoint, nil, true), 'hello')
    end)
    with_server(function(client)
        client:read({delimiter = '\r\n\r\n', chunk = 4096}, 2)
        client:write(
            'HTTP/1.1 200 OK\r\nContent-Length: 4\r\n'
                .. 'Connection: close\r\n\r\npong'
        )
    end, function(endpoint)
        t.assert_equals(invoke('http|' .. endpoint, nil, true), 'pong')
    end)
end

g.test_keepalive_options = function()
    with_server(function(client)
        client:read(1, 2)
    end, function(endpoint)
        t.assert_equals(invoke('tcp-options|' .. endpoint), 'ok')
    end)
end

g.test_write_failure_under_backpressure = function()
    with_server(function(client)
        t.assert_equals(client:read(1, 2), 'x')
        t.assert(client:linger(true, 0)) -- Reset after reading the first byte.
    end, function(endpoint)
        t.assert_equals(invoke('tcp-write-reset|' .. endpoint), 'write-error')
    end)
end

g.test_http_timeout_limitation = function()
    with_server(function(client)
        client:read({delimiter = '\r\n\r\n', chunk = 4096}, 2)
        fiber.sleep(0.15) -- Longer than Client.Timeout; always finish the test.
        client:write(
            'HTTP/1.1 200 OK\r\nContent-Length: 4\r\n'
                .. 'Connection: close\r\n\r\npong'
        )
    end, function(endpoint)
        t.assert_equals(invoke('http-timeout|' .. endpoint), 'timeout-ignored')
    end)
    observations.http_timeout = {
        status = 'known-limitation',
        scenario = 'TinyGo Client.Timeout does not bound HTTP I/O.',
    }
end
