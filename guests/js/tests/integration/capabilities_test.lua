local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')
local socket = require('socket')
local json = require('json')

local g = t.group('js_capabilities')
local root = fio.abspath('guests/js/tests/components')
local observations = {fresh = {}}

g.after_all(function()
    local file =
        assert(io.open('guests/js/dist/capabilities-observed.json', 'w'))
    file:write(json.encode(observations))
    file:close()
end)

local function with_module(path, options, fn)
    local module = wasm.load(fio.pathjoin(root, path), options)

    local ok, value = pcall(fn, module)

    wasm.drop(module)
    if not ok then
        error(value, 0)
    end

    return value
end

local function with_server(http, fn)
    local server = assert(socket.tcp_server('127.0.0.1', 0, function(client)
        if http then
            assert(client:read({delimiter = '\r\n\r\n', chunk = 4096}, 5))
            client:write(
                'HTTP/1.1 200 OK\r\nContent-Length: 4\r\n'
                .. 'Connection: close\r\n\r\npong'
            )
        else
            assert(client:read('\n', 5) == 'ping\n')
            client:write('pong')
        end
        client:close()
    end))

    local ok, value = pcall(fn, server:name().port)
    server:close()
    if not ok then
        error(value, 0)
    end

    return value
end

for _, profile in ipairs({'fresh'}) do
    g['test_' .. profile .. '_compression'] = function()
        with_module('builtins/' .. profile .. '.wasm', {}, function(module)
            local features = json.decode(wasm.call(module, 'features'))
            observations[profile].features = features
            observations[profile].compression =
                {'deflate', 'gzip', 'deflate-raw'}

            t.assert_equals(features.CompressionStream, 'function')
            t.assert_equals(features.DecompressionStream, 'function')

            for _, format in ipairs({'deflate', 'gzip', 'deflate-raw'}) do
                local input = {0, 1, 2, 255, 0, 77}
                local encoded = wasm.call(module, 'compress', format, input)

                t.assert_equals(
                    wasm.call(module, 'decompress', format, encoded),
                    input
                )
            end
        end)
    end

    g['test_' .. profile .. '_http'] = function()
        with_server(true, function(port)
            local url = 'http://127.0.0.1:' .. port .. '/'
            with_module(
                'builtins/' .. profile .. '-http.wasm',
                {},
                function(module)
                    t.assert_equals(
                        wasm.call(module, 'fetch-url', url),
                        '200:pong'
                    )
                end
            )
            -- Observation only: WASI HTTP and WASI socket policies may differ.
            with_module(
                'builtins/' .. profile .. '-http.wasm',
                {inherit_network = false, allow_tcp = false},
                function(module)
                    local ok, value = pcall(wasm.call, module, 'fetch-url', url)
                    observations[profile].http = {
                        localhost = '200:pong',
                        socket_policy = {success = ok, result = tostring(
                            value
                        )},
                    }
                end
            )
            with_module('builtins/' .. profile .. '.wasm', {}, function(module)
                local ok, value = pcall(wasm.call, module, 'fetch-url', url)
                observations[profile].http_disabled =
                    {success = ok, result = tostring(value)}

                t.assert_not(ok)
            end)
        end)
    end
end

g.test_node_zlib_requires_another_host_abi = function()
    local ok, value =
        pcall(wasm.load, fio.pathjoin(root, 'node-zlib/fresh.wasm'))
    if ok then
        wasm.drop(value)
    end

    t.assert_not(ok)
    t.assert_str_contains(tostring(value), 'jco:node/zlib')
    observations.node_zlib =
        {status = 'unsupported-host-import', detail = tostring(value)}
end

for _, profile in ipairs({'fresh'}) do
    g['test_' .. profile .. '_wasi_sockets_audit'] = function()
        with_server(false, function(port)
            with_module('sockets/' .. profile .. '.wasm', {}, function(module)
                local result = wasm.call(module, 'exchange', port)
                if result == 'pong' then
                    t.assert_equals(result, 'pong')
                    t.assert(
                        json.decode(wasm.call(module, 'resolve', 'localhost'))
                    )
                    observations[profile].sockets =
                        {status = 'tcp-and-dns-executed'}
                else
                    t.assert_equals(
                        result,
                        'binding-error: import_network_0_2_12$Network'
                            .. ' is not defined'
                    )
                    observations[profile].sockets = {
                        status = 'generated-binding-runtime-failure',
                        detail = result,
                    }
                end
            end)
        end)
    end
end

g.test_fresh_node_net_audit = function()
    local file =
        assert(io.open('guests/js/dist/node-net-build-status.txt', 'r'))
    local status = file:read('*a')
    file:close()
    if status:match('^known%-build%-failure:') then
        observations.node_net = {status = 'build-failure', detail = status}

        t.assert_not(fio.stat(fio.pathjoin(root, 'node-net/fresh.wasm')))
    else
        t.assert_equals(status, 'built\n')
        with_server(false, function(port)
            with_module('node-net/fresh.wasm', {}, function(module)
                t.assert_equals(wasm.call(module, 'exchange', port), 'pong')
            end)
        end)
        observations.node_net = {status = 'tcp-executed'}
    end
end

local https_url = os.getenv('TWR_JS_HTTPS_PROBE_URL')

if https_url ~= nil then
    g.test_fresh_https = function()
        with_module('builtins/fresh-http.wasm', {}, function(module)
            local result = wasm.call(module, 'fetch-url', https_url)

            t.assert_str_contains(result, '200:')
            observations.https =
                {url = https_url, status = 'executed', response = result}
        end)
    end
else
    observations.https = {
        status = 'not-run',
        reason = 'External HTTPS is opt-in; no external network in CI.',
    }
end
