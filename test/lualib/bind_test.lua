local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')

package.path = './test/?.lua;' .. package.path
local utils = require('utils')

local g = t.group('bind')

g.before_all(function(cg)
    local path = fio.pathjoin(
        utils.get_test_dir(), 'lualib', 'wasm_component_test',
        'component_exports_test', 'adder.wasm'
    )
    cg.module = wasm.load(path)
    cg.bound = wasm.bind(cg.module)
end)

g.after_all(function(cg)
    wasm.drop(cg.module)
end)

g.test_root_exports = function(cg)
    t.assert_type(cg.bound['t-string'], 'function')
    t.assert_equals(cg.bound['t-string']('hello'), 'hello')
    t.assert_equals(cg.bound['t-record']({x = 42, y = 'answer'}), {
        x = 42,
        y = 'answer',
    })
end

g.test_interface_exports = function(cg)
    local nums = cg.bound['docs:adder/nums@0.1.0']
    t.assert_type(nums, 'table')
    t.assert_equals(nums['t-bool'](true), true)
    t.assert_equals(nums['t-u64'](1234567890), 1234567890)
    t.assert_equals(nums['t-f64'](3.1415), 3.1415)
end

g.test_runtime_interfaces_are_hidden = function(cg)
    t.assert_equals(cg.bound['wasi:cli/run@0.2.3'], nil)
end

g.test_unknown_module = function()
    local ok, err = pcall(wasm.bind, 'missing-module')
    t.assert_not(ok)
    t.assert_str_contains(tostring(err), 'module `missing-module` not found')
end
