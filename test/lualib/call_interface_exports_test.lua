local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local g = t.group("call_interface_exports")

g.test_get_component_exports = function()
    local path = fio.pathjoin(utils.get_test_dir(), "lualib", "wasm_component_test", "component_exports_test", "adder.wasm")
    local m = wasm.load(path)

    local res = wasm.batch(m, {
        { "docs:adder/nums@0.1.0::t-bool", true },
        { "docs:adder/nums@0.1.0::t-u64", 1234567890 },
        { "docs:adder/nums@0.1.0::t-f64", 3.1415 },
    })
    
    t.assert_equals(res[1], true)
    t.assert_equals(res[2], 1234567890)
    t.assert_equals(res[3], 3.1415)
end
