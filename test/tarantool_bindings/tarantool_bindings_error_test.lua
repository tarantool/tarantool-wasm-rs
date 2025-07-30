local t = require('luatest')
local luawasm = require('luawasm')
local fio = require('fio')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local test_group = "bindings_error_test"

local g = t.group(
    test_group,
    t.helpers.matrix({
        lang = {"python", "go", "js", "c"}
    })
)

g.before_all(function(cg)
    local path = fio.pathjoin(utils.get_test_dir(), "tarantool_bindings", "wasm_component_test", test_group, cg.params.lang)
    local wasm
    if cg.params.lang == "c" then
        wasm = "adder.component.wasm"
    end
    cg.m = luawasm:new({
        dir = path,
        wasm = wasm,
    })
end)

-- error_new
g.test_error_new_without_context = function(cg)
    local err = cg.m:t_error_new("Connection timeout", "network", 504, nil, nil)

    t.assert_equals(type(err), "table")
    t.assert_equals(err.message, "Connection timeout")
    t.assert_equals(err.type, "network")
    t.assert_equals(err.code, 504)
    t.assert_equals(err.file, nil)
    t.assert_equals(err.line, nil)
end

g.test_error_new_with_context = function(cg)
    local err = cg.m:t_error_new("Invalid argument", "validation", 1001, "input.lua", 42)

    t.assert_equals(type(err), "table")
    t.assert_equals(err.message, "Invalid argument")
    t.assert_equals(err.type, "validation")
    t.assert_equals(err.code, 1001)
    t.assert_equals(err.file, "input.lua")
    t.assert_equals(err.line, 42)
end

-- set
g.test_error_set_and_last = function(cg)
    local res = cg.m:t_last()
    t.assert_equals(res, nil)

    local err = cg.m:t_error_new("Connection timeout", "network", 504, "input.lua", 42)
    res = cg.m:batch{
        { cg.m.t_set, err },
        { cg.m.t_last }
    }
    t.assert_equals(res[1], nil)

    local unpacked = res[2]

    t.assert_equals(unpacked.message, "Connection timeout")
    t.assert_equals(unpacked.code, 504)

    -- Note: when setting an error via `set`, the underlying implementation uses
    -- `tarantool::error::set_last_error` from the Tarantool `picodata` module.
    -- This function internally sets the `type` and `base_type` fields (usually to `ClientError`)
    -- and does not provide a way to override them manually.
    t.assert_equals(unpacked.type, "ClientError")
    t.assert_equals(unpacked.file, "input.lua")
    t.assert_equals(unpacked.line, 42)
end

-- clear
g.test_error_clear = function(cg)
    local err = cg.m:t_error_new("Connection timeout", "network", 504, "input.lua", 42)
    local res = cg.m:batch{
        { cg.m.t_clear },
        { cg.m.t_last },
        { cg.m.t_set, err },
        { cg.m.t_last },
        { cg.m.t_clear },
        { cg.m.t_last },
    }
    t.assert_equals(res[1], nil)
    t.assert_equals(res[2], nil)
    t.assert_equals(res[3], nil)
    t.assert_type(res[4], "table")
    t.assert_equals(res[5], nil)
    t.assert_equals(res[6], nil)
end

-- to_string
g.test_error_to_string = function(cg)
    local err = cg.m:t_error_new("Connection timeout", "network", 504, nil, nil)
    local err_string = cg.m:t_to_string(err)
    t.assert_equals(
        err_string,
        "[504] network: Connection timeout"
    )
end

