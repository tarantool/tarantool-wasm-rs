local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')
local read_new_log = utils.read_new_log

local test_group = "bindings_txn_test"

local g = t.group(
    test_group,
    t.helpers.matrix({
        lang = {"python", "go", "js", "c"}
    })
)

g.before_all(function(cg)
    local path =
    fio.pathjoin(utils.get_test_dir(), "tarantool_bindings",
                 "wasm_component_test", test_group, cg.params.lang)
    if cg.params.lang == "c" then
        path = fio.pathjoin(path, "adder.component.wasm")
    else
        path = fio.pathjoin(path, "adder.wasm")
    end
    cg.m = wasm.load(path)

    box.cfg{}
end)

g.after_all(function()
    utils.cleanup_logs(fio.cwd())
end)

local expected_lines = {
    "unknown say%.rs:%d+ I> transaction id before begin: none",
    "unknown say%.rs:%d+ I> transaction%.begin%(%)",
    "unknown say%.rs:%d+ I> transaction id first: %d+",
    "unknown say%.rs:%d+ I> transaction isolation: read%-confirmed",
    "unknown say%.rs:%d+ I> transaction%.make_synchronous%(%)",
    "unknown say%.rs:%d+ I> transaction%.set_isolation%(%)",
    "unknown say%.rs:%d+ I> transaction isolation after set: best%-effort",
    "unknown say%.rs:%d+ I> transaction%.commit%(%)",
    "unknown say%.rs:%d+ I> transaction%.begin%(%) second",
    "unknown say%.rs:%d+ I> transaction id second: %d+",
    "unknown say%.rs:%d+ I> transaction%.rollback%(%)",
    "unknown say%.rs:%d+ I> transaction id after rollback: none",
}

local function normalize_lines(log)
    local lines = {}
    for line in log:gmatch("[^\r\n]+") do
        table.insert(lines, line)
    end
    return lines
end

local function strip_prefix(line)
    local _, _, clean = line:find("(%w-/.- say%.rs:.*)")
    return clean or line
end

local function match_line(actual, pattern)
    local cleaned = strip_prefix(actual)
    return cleaned:match(pattern) ~= nil
end

local function assert_expected_log_lines(content)
    local lines = normalize_lines(content)

    for _, expected in ipairs(expected_lines) do
        local found = false
        for _, actual in ipairs(lines) do
            if match_line(actual, expected) then
                found = true
                break
            end
        end
        t.assert(found, ("Expected log line not found: %s"):format(expected))
    end
end

g.test_txn_lifecycle = function(cg)
    local get_new_log = read_new_log()

    local handle = wasm.run(cg.m)
    local res = wasm.join(handle)
    t.assert(res)

    t.helpers.retrying({timeout = 2, delay = 0.01}, function()
        assert_expected_log_lines(get_new_log())
    end)
end
