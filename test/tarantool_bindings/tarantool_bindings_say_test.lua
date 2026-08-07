local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')

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

    local path =
    fio.pathjoin(utils.get_test_dir(), "tarantool_bindings",
                 "wasm_component_test", test_group, cg.params.lang)
    if cg.params.lang == "c" then
        path = fio.pathjoin(path, "adder.component.wasm")
    else
        path = fio.pathjoin(path, "adder.wasm")
    end
    cg.m = wasm.load(path)
end)

local function log(cg, ctx)
    wasm.call(cg.m, "t-say", "info", "hello from t_say", ctx)
    wasm.call(cg.m, "t-say-error", "an error occurred", ctx)
    wasm.call(cg.m, "t-say-crit", "critical message", ctx)
    wasm.call(cg.m, "t-say-warn", "warning message", ctx)
    wasm.call(cg.m, "t-say-info", "info message", ctx)
    wasm.call(cg.m, "t-say-verbose", "verbose output", ctx)
    wasm.call(cg.m, "t-say-debug", "debug output", ctx)
end

local function wait_for_log(get_new_log, marker)
    return t.helpers.retrying({timeout = 2, delay = 0.01}, function()
        local content = get_new_log()
        t.assert_str_contains(content, marker)
        return content
    end)
end

g.test_say_interfaces = function(cg)
    local get_new_log = read_new_log()

    local ok, res = pcall(log, cg)
    t.assert(ok)

    local content = wait_for_log(get_new_log, "D> debug output")

    t.assert_str_contains(content, "I> hello from t_say")
    t.assert_str_contains(content, "E> an error occurred")
    t.assert_str_contains(content, "C> critical message")
    t.assert_str_contains(content, "W> warning message")
    t.assert_str_contains(content, "I> info message")
    t.assert_str_contains(content, "V> verbose output")
    t.assert_str_contains(content, "D> debug output")
end

g.test_say_interfaces_with_context = function(cg)
    local get_new_log = read_new_log()
    local ctx = { file = "some_file.lua", line = 42 }
    local ok, res = pcall(log, cg, ctx)
    t.assert(ok)

    local content = wait_for_log(
        get_new_log,
        "some_file.lua:42 D> debug output"
    )

    t.assert_str_contains(content, "some_file.lua:42 I> hello from t_say")
    t.assert_str_contains(content, "some_file.lua:42 E> an error occurred")
    t.assert_str_contains(content, "some_file.lua:42 C> critical message")
    t.assert_str_contains(content, "some_file.lua:42 W> warning message")
    t.assert_str_contains(content, "some_file.lua:42 I> info message")
    t.assert_str_contains(content, "some_file.lua:42 V> verbose output")
    t.assert_str_contains(content, "some_file.lua:42 D> debug output")
end
