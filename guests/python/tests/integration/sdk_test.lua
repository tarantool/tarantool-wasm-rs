local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')
local g = t.group('python_sdk')

g.test_embedded_zlib_without_ssl_host_import = function()
    local module = wasm.load(fio.abspath(
        'guests/python/tests/components/stdlib_smoke/stdlib.wasm'), {
        inherit_network = false,
    })
    local ok, result = pcall(wasm.call, module, 'round-trip', {0, 255, 1, 254})
    wasm.drop(module)
    t.assert(ok, tostring(result))
    t.assert_equals(result, {0, 255, 1, 254})
end
