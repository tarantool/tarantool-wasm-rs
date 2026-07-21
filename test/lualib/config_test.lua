local t = require("luatest")
local wasm = require("wasm")
local fio = require("fio")

package.path = "./test/?.lua;" .. package.path
local utils = require("utils")

local g = t.group("config_test")

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

local wasm_path = fio.pathjoin(utils.get_test_dir(), "lualib", "wasm_component_test", "config_test", "adder.wasm")

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
