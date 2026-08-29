local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local g = t.group("component_limits")
local path = fio.pathjoin(utils.get_test_dir(), "lualib", "wasm_component_test", "component_limits_test", "adder.wasm")

g.before_all(function(cg)
    cg.unlimited_module = wasm.load(path)
end)

g.after_all(function(cg)
    wasm.drop(cg.unlimited_module)
end)

g.test_fuel_no_limit = function(cg)
    local ok, _ = pcall(wasm.call, cg.unlimited_module, "foo")
    t.assert(ok)
end

g.test_mem_size_no_limit = function(cg)
    local ok, _ = pcall(wasm.call, cg.unlimited_module, "bar")
    t.assert(ok)
end

g.test_fuel_limit = function(cg)
    local before = wasm.stats()
    local m = wasm.load(path, { max_instructions = 50000000 })
    local ok, err = pcall(wasm.call, m, "foo")
    t.assert_not(ok)
    t.assert_str_contains(tostring(err), "error while executing at wasm backtrace")
    local after = wasm.stats()
    t.assert_ge(after.fuel.executions, before.fuel.executions + 1)
    t.assert_gt(after.fuel.consumed, before.fuel.consumed)
    t.assert_ge(after.traps.total, before.traps.total + 1)
    t.assert_ge(after.traps.by_kind.out_of_fuel, 1)
    local recovered, recovery_err = pcall(wasm.call, cg.unlimited_module, "foo")
    t.assert(recovered, recovery_err)
    t.assert(wasm.drop(m))
end

g.test_mem_size_limit = function()
    local m = wasm.load(path, { memory_limit = 16*1024*1024 })
    local ok, err = pcall(wasm.call, m, "bar")
    t.assert_not(ok)
    t.assert_str_contains(tostring(err), "error while executing at wasm backtrace")
    t.assert(wasm.drop(m))
end
