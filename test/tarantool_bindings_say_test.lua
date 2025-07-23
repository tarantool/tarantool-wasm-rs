local t = require('luatest')
local luawasm = require('luawasm')
local fio = require('fio')
local fiber = require('fiber')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')
local read_new_log = utils.read_new_log

local test_group = "bindings_say_test"

local g = t.group(
    test_group,
    t.helpers.matrix({
        lang = {"python", "c", "js", "go"}
    })
)

g.before_all(function(cg)
    box.cfg{log_level = 7}

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

local function log(cg, ctx)
    cg.m:t_say("s-info", "hello from t_say", ctx)
    cg.m:t_say_error("an error occurred", ctx)
    cg.m:t_say_crit("critical message", ctx)
    cg.m:t_say_warn("warning message", ctx)
    cg.m:t_say_info("info message", ctx)
    cg.m:t_say_verbose("verbose output", ctx)
    cg.m:t_say_debug("debug output", ctx)
    cg.m:t_say_syserror("sys error occurred", ctx)
end

g.test_say_interfaces = function(cg)
    local get_new_log = read_new_log()

    local ok, res = pcall(log, cg)
    t.assert(ok)

    fiber.sleep(0.1)

    local content = get_new_log()

    t.assert_str_contains(content, "I> hello from t_say")
    t.assert_str_contains(content, "E> an error occurred")
    t.assert_str_contains(content, "C> critical message")
    t.assert_str_contains(content, "W> warning message")
    t.assert_str_contains(content, "I> info message")
    t.assert_str_contains(content, "V> verbose output")
    t.assert_str_contains(content, "D> debug output")
    t.assert_str_contains(content, "!> sys error occurred")
end

g.test_say_interfaces_with_context = function(cg)
    local ctx = { filename = "some_file.lua", line = 42 }
    local ok, res = pcall(log, cg, ctx)
    t.assert(ok)

    fiber.sleep(0.1)

    local path = "luatest.log"
    local fh = assert(io.open(path, "r"))
    local content = fh:read("*a")
    fh:close()

    t.assert_str_contains(content, "some_file.lua:42 I> hello from t_say")
    t.assert_str_contains(content, "some_file.lua:42 E> an error occurred")
    t.assert_str_contains(content, "some_file.lua:42 C> critical message")
    t.assert_str_contains(content, "some_file.lua:42 W> warning message")
    t.assert_str_contains(content, "some_file.lua:42 I> info message")
    t.assert_str_contains(content, "some_file.lua:42 V> verbose output")
    t.assert_str_contains(content, "some_file.lua:42 D> debug output")
    t.assert_str_contains(content, "some_file.lua:42 !> sys error occurred")
end
