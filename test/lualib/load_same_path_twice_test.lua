local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local g = t.group("load_same_path")

g.test_load_same_path_twice = function()
    local path = fio.pathjoin(utils.get_test_dir(), "lualib", "wasm_component_test", "component_exports_test", "adder.wasm")
    local m1 = wasm.load(path)
    local m2 = wasm.load(path)
    t.assert_not_equals(m1, m2)

    t.assert_equals(wasm.call(m1, "docs:adder/nums@0.1.0::t-s16", 1), 1)
    t.assert_equals(wasm.call(m2, "docs:adder/nums@0.1.0::t-s16", 2), 2)

    wasm.drop(m1)
    wasm.drop(m2)
end
