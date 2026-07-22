local t = require("luatest")
local wasm = require("wasm")
local fio = require("fio")

package.path = "./test/?.lua;" .. package.path
local utils = require("utils")

local g = t.group("config_test")
local wasm_path

local function write_file(path, content)
    local f = assert(io.open(path, "w"))
    f:write(content)
    f:close()
end

local function read_file(path)
    local f = assert(io.open(path, "r"))
    local data = f:read("*a")
    f:close()
    return data
end

local function assert_load_error(config, expected)
    local ok, result = pcall(wasm.load, wasm_path, config)
    if ok then
        wasm.drop(result)
    end
    t.assert_not(ok)
    t.assert_str_contains(tostring(result), expected)
end

local function assert_call_error(config, expected)
    local m = wasm.load(wasm_path, config)
    local ok, err = pcall(wasm.call, m, "t-inherit-env")
    wasm.drop(m)
    t.assert_not(ok)
    t.assert_str_contains(tostring(err), expected)
end

wasm_path = fio.pathjoin(utils.get_test_dir(), "lualib", "wasm_component_test", "config_test", "adder.wasm")

g.before_all(function(cg)
    cg.tmpdir = fio.tempdir()
    cg.in_path = fio.pathjoin(cg.tmpdir, "in.txt")
    cg.out_path = fio.pathjoin(cg.tmpdir, "out.txt")
    cg.err_path = fio.pathjoin(cg.tmpdir, "err.txt")
    cg.ro_dir = fio.pathjoin(cg.tmpdir, "ro")
    cg.rw_dir = fio.pathjoin(cg.tmpdir, "rw")

    fio.mkdir(cg.ro_dir)
    fio.mkdir(cg.rw_dir)
    write_file(cg.in_path, "42\n")
    write_file(fio.pathjoin(cg.ro_dir, "f1.txt"), "ro content")

    cg.module = wasm.load(wasm_path, {
        stdin = cg.in_path,
        stdout = cg.out_path,
        stderr = cg.err_path,
        env = {
            TEST_VAR = "ABC",
        },
        args = {1, "abc"},
        preopened_dirs = {
            {cg.ro_dir, "./ro", perms = "ro"},
            {cg.rw_dir, "./rw", perms = "rw"},
        },
    })
end)

g.after_all(function(cg)
    wasm.drop(cg.module)
end)

g.test_stdout = function(cg)
    wasm.call(cg.module, "t-stdout", "hello from wasm")
    local data = read_file(cg.out_path)
    t.assert_str_contains(data, "hello from wasm")
end

g.test_stdin = function(cg)
    wasm.call(cg.module, "t-stdin")
    local out = read_file(cg.out_path)
    t.assert_str_contains(out, "42")
end

g.test_stderr = function(cg)
    wasm.call(cg.module, "t-stderr")
    local err = read_file(cg.err_path)
    t.assert_str_contains(err, "error!")
end

g.test_env = function(cg)
    local res = wasm.call(cg.module, "t-env")
    t.assert_equals(res, "ABC")
end

g.test_preopen = function(cg)
    wasm.call(cg.module, "t-preopen")

    local out = read_file(cg.out_path)
    t.assert_str_contains(out, "./ro/f1.txt RO: ro content")
    t.assert_str_contains(out, "RO FAIL-2")

    local written = read_file(fio.pathjoin(cg.rw_dir, "f2.txt"))
    t.assert_str_contains(written, "./rw/f2.txt OK")
end

g.test_args = function(cg)
    wasm.call(cg.module, "t-args")
    local out = read_file(cg.out_path)
    t.assert_str_contains(out, "['1', 'abc']")
end

g.test_default_values = function()
    local m = wasm.load(wasm_path, {})
    local res = wasm.call(m, "t-inherit-env")
    t.assert_not_equals(res, "HOME IS NONE")

    t.assert(wasm.drop(m))

    m = wasm.load(wasm_path, { inherit_env = false })
    res = wasm.call(m, "t-inherit-env")
    t.assert_equals(res, "HOME IS NONE")
    t.assert(wasm.drop(m))
end

g.test_invalid_field_types = function()
    assert_load_error({inherit_env = "yes"}, "boolean")
    assert_load_error({args = "one"}, "table")
    assert_load_error({stdin = true}, "string")
    assert_load_error({env = "A=B"}, "table")
    assert_load_error({allowed_ips = "127.0.0.1"}, "table")
    assert_load_error({preopened_dirs = "./data"}, "table")
    assert_load_error({memory_limit = "large"}, "integer")
end

g.test_invalid_ip = function()
    assert_load_error({allowed_ips = {"not-an-ip"}}, "invalid IP address")
end

g.test_invalid_preopen_permissions = function(cg)
    assert_load_error({
        preopened_dirs = {
            {cg.ro_dir, "./ro", perms = "readonly"},
        },
    }, "invalid preopened directory permissions")
end

g.test_stream_and_preopen_errors = function(cg)
    local missing = fio.pathjoin(cg.tmpdir, "missing")
    assert_call_error({stdin = missing}, "failed to read stdin")
    assert_call_error({stdout = fio.pathjoin(missing, "stdout")}, "failed to open stdout")
    assert_call_error({stderr = fio.pathjoin(missing, "stderr")}, "failed to open stderr")
    assert_call_error({
        preopened_dirs = {
            {missing, "./missing", perms = "ro"},
        },
    }, "failed to preopen directory")
end

g.test_session_setup_error = function(cg)
    local missing = fio.pathjoin(cg.tmpdir, "missing-session-stdin")
    local m = wasm.load(wasm_path)
    local ok, err = pcall(wasm.open_session, m, {stdin = missing})
    if ok then
        wasm.close_session(err)
    end
    wasm.drop(m)
    t.assert_not(ok)
    t.assert_str_contains(tostring(err), "failed to initialize session")
    t.assert_str_contains(tostring(err), "failed to read stdin")
end

g.test_run_setup_error = function(cg)
    local run_path = fio.pathjoin(
        utils.get_test_dir(), "lualib", "wasm_component_test",
        "cancel_run_test", "adder.component.wasm"
    )
    local missing = fio.pathjoin(cg.tmpdir, "missing-run-stdin")
    local m = wasm.load(run_path)
    local ok, result = pcall(wasm.run, m, {stdin = missing})
    if ok then
        pcall(wasm.join, result)
    end
    wasm.drop(m)
    t.assert_not(ok)
    t.assert_str_contains(tostring(result), "failed to read stdin")
end
