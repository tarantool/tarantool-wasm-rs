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

g.before_each(function(cg)
    cg.tmpdir = fio.tempdir()
end)

local wasm_path = fio.pathjoin(utils.get_test_dir(), "lualib", "wasm_component_test", "config_test", "adder.wasm")

g.test_stdout = function(cg)
    local out = fio.pathjoin(cg.tmpdir, "out.txt")

    local m = wasm.load(wasm_path, {
        stdout = out
    })

    wasm.call(m, "t-stdout", "hello from wasm")
    local data = read_file(out)
    t.assert_str_contains(data, "hello from wasm")
end

g.test_stdin = function(cg)
    local in_path = fio.pathjoin(cg.tmpdir, "in.txt")
    local out_path = fio.pathjoin(cg.tmpdir, "out.txt")

    write_file(in_path, "42\n")

    local m = wasm.load(wasm_path, {
        stdin = in_path,
        stdout = out_path
    })

    wasm.call(m, "t-stdin")
    local out = read_file(out_path)
    t.assert_str_contains(out, "42")
end

g.test_stderr = function(cg)
    local err_path = fio.pathjoin(cg.tmpdir, "err.txt")

    local m = wasm.load(wasm_path, {
        stderr = err_path
    })

    wasm.call(m, "t-stderr")
    local err = read_file(err_path)
    t.assert_str_contains(err, "error!")
end

g.test_env = function(cg)
    local m = wasm.load(wasm_path, {
        env = {
            TEST_VAR = "ABC",
        }
    })

    local res = wasm.call(m, "t-env")
    t.assert_equals(res, "ABC")
end

g.test_preopen = function(cg)
    local ro_dir = fio.pathjoin(cg.tmpdir, "ro")
    local rw_dir = fio.pathjoin(cg.tmpdir, "rw")
    fio.mkdir(ro_dir)
    fio.mkdir(rw_dir)

    write_file(fio.pathjoin(ro_dir, "f1.txt"), "ro content")

    local m = wasm.load(wasm_path, {
        stdout = fio.pathjoin(cg.tmpdir, "out.txt"),
        preopened_dirs = {
            {ro_dir, "./ro", perms = "ro"},
            {rw_dir, "./rw", perms = "rw"},
        }
    })

    wasm.call(m, "t-preopen")

    local out = read_file(fio.pathjoin(cg.tmpdir, "out.txt"))
    t.assert_str_contains(out, "./ro/f1.txt RO: ro content")
    t.assert_str_contains(out, "RO FAIL-2")

    local written = read_file(fio.pathjoin(rw_dir, "f2.txt"))
    t.assert_str_contains(written, "./rw/f2.txt OK")
end

g.test_args = function(cg)
    local out_path = fio.pathjoin(cg.tmpdir, "out.txt")

    local m = wasm.load(wasm_path, {
        stdout = out_path,
        args = {1, "abc"},
    })

    wasm.call(m, "t-args")
    local out = read_file(out_path)
    t.assert_str_contains(out, "['1', 'abc']")
end

g.test_default_values = function(cg)
    local m = wasm.load(wasm_path, {})
    local res = wasm.call(m, "t-inherit-env")
    t.assert_not_equals(res, "HOME IS NONE")

    res = wasm.drop(m)
    t.assert(res)

    m = wasm.load(wasm_path, { inherit_env = false })
    local res = wasm.call(m, "t-inherit-env")
    t.assert_equals(res, "HOME IS NONE")
end

g.test_instance_pool_with_prewarm = function(cg)
    local m = wasm.load(wasm_path, {
        env = {
            TEST_VAR = "POOL-OK",
        },
        pool_size = 2,
        warm_instances = 2,
    })

    t.assert_equals(wasm.call(m, "t-env"), "POOL-OK")
    t.assert_equals(wasm.call(m, "t-env"), "POOL-OK")
    t.assert_equals(wasm.call(m, "t-env"), "POOL-OK")
end

g.test_instance_pool_fallback_to_on_demand = function(cg)
    local m = wasm.load(wasm_path, {
        env = {
            TEST_VAR = "FALLBACK-OK",
        },
        pool_size = 1,
        warm_instances = 0,
    })

    t.assert_equals(wasm.call(m, "t-env"), "FALLBACK-OK")
    t.assert_equals(wasm.call(m, "t-env"), "FALLBACK-OK")
end

g.test_instance_pool_batch = function(cg)
    local m = wasm.load(wasm_path, {
        env = {
            TEST_VAR = "BATCH-POOL-OK",
        },
        pool_size = 1,
        warm_instances = 1,
    })

    local res = wasm.batch(m, {
        { "t-env" },
        { "t-env" },
    })

    t.assert_equals(res[1], "BATCH-POOL-OK")
    t.assert_equals(res[2], "BATCH-POOL-OK")

    local res2 = wasm.batch(m, {
        { "t-env" },
    })
    t.assert_equals(res2[1], "BATCH-POOL-OK")
end
