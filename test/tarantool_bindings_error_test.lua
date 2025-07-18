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
    local path = fio.pathjoin(utils.get_test_dir(), "wasm_component_test", test_group, cg.params.lang)
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
g.test_error_set = function(cg)
    t.skip("Skipped: box.error.set() no longer affects WASM function context after 'tt thread' patch. cg.m:t_last() always returns nil")

    local err = cg.m:t_error_new("Connection timeout", "network", 504, "input.lua", 42)
    cg.m:t_set(err)

    local box_error_last = box.error.last()
    t.assert_equals(type(box_error_last), "cdata", "box.error.last() must return a cdata object")

    local unpacked = box_error_last:unpack()

    t.assert_equals(unpacked.message, "Connection timeout")
    t.assert_equals(unpacked.code, 504)

    -- Note: when setting an error via `set`, the underlying implementation uses
    -- `tarantool::error::set_last_error` from the Tarantool `picodata` module.
    -- This function internally sets the `type` and `base_type` fields (usually to `ClientError`)
    -- and does not provide a way to override them manually.
    t.assert_equals(unpacked.type, "ClientError")
    t.assert_equals(unpacked.base_type, "ClientError")

    t.assert_equals(unpacked.message, "Connection timeout")
    t.assert_equals(unpacked.trace[1].file, "input.lua")
    t.assert_equals(unpacked.trace[1].line, 42)
end

-- last
g.test_error_last = function(cg)
    t.skip("Skipped: box.error.set() no longer affects WASM function context after 'tt thread' patch. cg.m:t_last() always returns nil")

    local err

    err = cg.m:t_last()
    t.assert_equals(err, nil)

    local info = debug.getinfo(1, "Sl")
    local expected_file = info.short_src

    local line_set_error = debug.getinfo(1, "l").currentline + 1
    local box_error = box.error.new({ code = 500, reason = "Custom error 1" })
    box.error.set(box_error)

    err = cg.m:t_last()

    t.assert_equals(err.message, "Custom error 1")
    t.assert_equals(err.code, 500)
    t.assert_equals(err.type, "ClientError")
    t.assert_str_contains(err.file, expected_file)
    t.assert_equals(err.line, line_set_error)
end

-- clear
g.test_error_clear = function(cg)
    t.skip("Skipped: box.error.set() no longer affects WASM function context after 'tt thread' patch. cg.m:t_last() always returns nil")

    local err

    cg.m:t_clear()
    err = cg.m:t_last()
    t.assert_equals(err, nil)

    local info = debug.getinfo(1, "Sl")
    local expected_file = info.short_src
    local line_set_error = debug.getinfo(1, "l").currentline + 1
    local box_error = box.error.new({ code = 500, reason = "Custom error 1" })
    box.error.set(box_error)

    err = cg.m:t_last()

    t.assert_equals(err.message, "Custom error 1")
    t.assert_equals(err.code, 500)
    t.assert_equals(err.type, "ClientError")
    t.assert_str_contains(err.file, expected_file)
    t.assert_equals(err.line, line_set_error)

    cg.m:t_clear()
    err = cg.m:t_last()
    t.assert_equals(err, nil)
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

