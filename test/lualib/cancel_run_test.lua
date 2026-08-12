local t = require("luatest")
local wasm = require("wasm")
local fio = require("fio")

package.path = "./test/?.lua;" .. package.path
local utils = require("utils")

local g = t.group("cancel_run_test")

local function read_file(path)
    local f = assert(io.open(path, "r"))
    local data = f:read("*a")
    f:close()
    return data
end

local function count_lines(s)
    if s == "" then return 0 end
    local n = 0
    for _ in s:gmatch("[^\n]+") do
        n = n + 1
    end
    return n
end

local function wait_for_lines(path, min_lines)
    return t.helpers.retrying({timeout = 5, delay = 0.01}, function()
        local data = read_file(path)
        local lines = count_lines(data)
        t.assert_ge(lines, min_lines)
        return lines
    end)
end

local function wait_for_output(path, expected)
    t.helpers.retrying({timeout = 5, delay = 0.01}, function()
        t.assert_equals(read_file(path), expected)
    end)
end

local function assert_user_data_destructed(fn)
    t.assert_error_msg_matches(
        ".*bad argument #1.*userdata has been destructed.*",
        fn
    )
end

g.before_each(function(cg)
    cg.tmpdir = fio.tempdir()
end)

local wasm_path = fio.pathjoin(
    utils.get_test_dir(),
    "lualib", "wasm_component_test", "cancel_run_test", "adder.component.wasm"
)

g.test_cancel_long_running_task = function(cg)
    local out = fio.pathjoin(cg.tmpdir, "out.txt")

    local m = wasm.load(wasm_path, {
        stdout = out,
        env = { INF = "True" },
    })

    -- First run: start and cancel quickly
    local before = wasm.stats()
    local handle = wasm.run(m)
    t.assert_equals(wasm.stats().current.runs, before.current.runs + 1)
    local n1 = wait_for_lines(out, 1)
    t.assert(wasm.cancel(handle))
    t.assert_equals(wasm.stats().current.runs, before.current.runs)

    -- After cancel, join() on the same handle must fail
    assert_user_data_destructed(function() wasm.join(handle) end)

    -- Second run: let it print more, then cancel
    handle = wasm.run(m)
    local n2 = wait_for_lines(out, n1 + 1)
    t.assert(wasm.cancel(handle))

    t.assert_gt(n2, n1)
end

g.test_cancel_and_join_finished_task = function(cg)
    local out = fio.pathjoin(cg.tmpdir, "out.txt")

    local m = wasm.load(wasm_path, {
        stdout = out,
    })

    -- 1) Run, wait for its output, then cancel the finished task
    local handle = wasm.run(m)
    wait_for_output(out, "2\n")
    t.assert(wasm.cancel(handle)) -- should be no-op since task is done

    local res = read_file(out)
    t.assert_equals(res, "2\n")

    assert_user_data_destructed(function() wasm.join(handle) end)

    -- 2) Run again and join normally
    handle = wasm.run(m)
    t.assert(wasm.join(handle))

    res = read_file(out)
    t.assert_equals(res, "2\n")

    assert_user_data_destructed(function() wasm.cancel(handle) end)
end

g.test_join_propagates_guest_trap = function()
    local m = wasm.load(wasm_path, {
        env = { TRAP = "True" },
    })
    local handle = wasm.run(m)

    local ok, err = pcall(wasm.join, handle)

    t.assert_not(ok)
    t.assert_str_contains(tostring(err), "wasm")
    t.assert(wasm.drop(m))
end

g.test_join_accepts_zero_exit = function()
    local m = wasm.load(wasm_path, {
        env = { EXIT_CODE = "0" },
    })
    local handle = wasm.run(m)
    t.assert(wasm.join(handle))
    t.assert(wasm.drop(m))
end

g.test_join_reports_nonzero_exit = function()
    local m = wasm.load(wasm_path, {
        env = { EXIT_CODE = "7" },
    })
    local handle = wasm.run(m)
    local ok, err = pcall(wasm.join, handle)
    t.assert_not(ok)
    -- The Preview 1 command adapter maps every non-zero proc_exit status to
    -- wasi:cli/exit failure, represented by Wasmtime as I32Exit(1).
    t.assert_str_contains(tostring(err), "WASI command exited with status 1")
    t.assert(wasm.drop(m))
end
