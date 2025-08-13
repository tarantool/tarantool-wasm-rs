local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local g = t.group("drop_component")

g.test_drop_component = function()
    local path = fio.pathjoin(utils.get_test_dir(), "lualib", "wasm_component_test", "component_exports_test", "adder.wasm")
    local m = wasm.load(path)
    local ok, err, res

    res = wasm.call(m, "docs:adder/nums@0.1.0::t-bool", true)
    t.assert_equals(res, true)

    res = wasm.drop(m)
    t.assert(res)

    ok, err = pcall(wasm.call, m, "docs:adder/nums@0.1.0::t-bool", true)
    t.assert_not(ok)
    t.assert_str_contains(tostring(err), "not found")

    ok, err = pcall(wasm.drop, "unknown-module-uid")
    t.assert_not(ok)
    t.assert_str_contains(tostring(err), "module `unknown-module-uid` not found")
end
