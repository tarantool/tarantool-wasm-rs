local t = require('luatest')
local luawasm = require('luawasm')
local fio = require('fio')
local fiber = require('fiber')

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
    local path = fio.pathjoin(utils.get_test_dir(), "tarantool_bindings", "wasm_component_test", test_group, cg.params.lang)
    local wasm
    if cg.params.lang == "c" then
        wasm = "adder.component.wasm"
    end
    cg.m = luawasm:new({
        dir = path,
        wasm = wasm,
    })
    box.cfg{}
end)


g.after_all(function()
    utils.cleanup_logs(fio.cwd())
end)

local expected_lines = {
    "unknown say%.rs:%d+ I> txn%(%).*before begin: False",
    "main/%d+/main say%.rs:%d+ I> txn%.begin%(%)*",
    "main/%d+/main say%.rs:%d+ I> txn_id first: %d+",
    "main/%d+/main say%.rs:%d+ I> txn_isolation: TXN_ISOLATION_READ_CONFIRMED",
    "main/%d+/main say%.rs:%d+ I> txn_make_sync%(%)*",
    "main/%d+/main say%.rs:%d+ I> txn_isolation after set: TXN_ISOLATION_BEST_EFFORT",
    "unknown say%.rs:%d+ I> txn%.commit%(%)*",
    "main/%d+/main say%.rs:%d+ I> txn%.begin%(%)* second",
    "main/%d+/main say%.rs:%d+ I> txn_id second: %d+",
    "unknown say%.rs:%d+ I> txn%.rollback%(%)*",
    "unknown say%.rs:%d+ I> txn%(%).*after rollback: False",
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


g.test_txn_lifecycle = function(cg)
    local get_new_log = read_new_log()

    cg.m:run()
    local res = cg.m:join()
    t.assert(res)

    fiber.sleep(0.05)
    
    local content = get_new_log()
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

