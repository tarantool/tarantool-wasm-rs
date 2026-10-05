local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')
local g = t.group('ssl_asyncio')

local function load_interpreter(cg, network)
    cg.module = wasm.load(fio.abspath('guests/python/interpreter/adder.wasm'), {
        interpreter = true, inherit_network = network or false,
        allowed_ips = {'127.0.0.1'},
        preopened_dirs = {{fio.abspath('guests/python/tests/fixtures/ssl'), '/certs', perms = 'ro'}},
    })
    local file = assert(fio.open('guests/python/tests/integration/ssl_asyncio_cases.py', {'O_RDONLY'}))
    local script = file:read()
    file:close()
    t.assert_equals(wasm.interpreter_exec(cg.module, '__name__ = "ssl_asyncio_cases"\n' .. script), true)
end

g.before_each(function(cg)
    load_interpreter(cg, false)
end)

g.after_each(function(cg)
    if cg.module then wasm.drop(cg.module) end
end)

g.test_ssl_protocol = function(cg)
    for _, tls12 in ipairs({false, true}) do
        for _, buffered in ipairs({false, true}) do
            t.assert_equals(wasm.interpreter_call(cg.module, 'asyncio_ssl_protocol', '/certs', tls12, buffered),
                            'asyncio-ssl-protocol')
        end
    end
end

g.test_tls_server = function(cg)
    wasm.drop(cg.module)
    load_interpreter(cg, true)
    for _, tls12 in ipairs({false, true}) do
        t.assert_equals(wasm.interpreter_call(cg.module, 'asyncio_tls_server', '/certs', tls12, false),
                        'asyncio-tls-server')
    end
end

g.test_start_tls = function(cg)
    wasm.drop(cg.module)
    load_interpreter(cg, true)
    for _, tls12 in ipairs({false, true}) do
        t.assert_equals(wasm.interpreter_call(cg.module, 'asyncio_tls_server', '/certs', tls12, true),
                        'asyncio-tls-server')
    end
end
