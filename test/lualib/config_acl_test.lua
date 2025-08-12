local t = require("luatest")
local wasm = require("wasm")
local fio = require("fio")
local fiber = require('fiber')

package.path = "./test/?.lua;" .. package.path
local utils = require("utils")

local g = t.group("config_acl_test")

local function read_file(path)
    local f = assert(io.open(path, "r"))
    local data = f:read("*a")
    f:close()
    return data
end

local wasm_path = fio.pathjoin(utils.get_test_dir(), "lualib", "wasm_component_test", "config_acl_test", "adder.wasm")

local m = wasm.load(wasm_path, {
    allowed_ips = { "127.0.0.1", "127.1.2.3" },
    allowed_ports = { 8000, 8080 },
})

g.test_acl_allow_localhost_8000 = function()
    local handler = wasm.run(m, {
        inherit_stdout = false,
        args = { "127.0.0.1", "8000" }
    })
    fiber.sleep(3)

    local res = wasm.call(m, "foo", "127.0.0.1", 8000)
    t.assert(res)
    t.assert(wasm.join(handler))
end

g.test_acl_allow_127_1_2_3_8080 = function()
    local handler = wasm.run(m, {
        inherit_stdout = false,
        args = { "127.1.2.3", "8080" }
    })
    fiber.sleep(3)

    local res = wasm.call(m, "foo", "127.1.2.3", 8080)
    t.assert(res)
    t.assert(wasm.join(handler))
end

g.test_acl_deny_localhost_1234 = function()
    local handler = wasm.run(m, {
        inherit_stdout = false,
        args = { "127.0.0.1", "1234" }
    })
    fiber.sleep(3)

    local res = wasm.call(m, "foo", "127.0.0.1", 1234)
    t.assert_not(res)
    t.assert(wasm.join(handler))
end

g.test_acl_deny_127_1_2_4_8080 = function()
    local handler = wasm.run(m, {
        inherit_stdout = false,
        args = { "127.1.2.4", "8080" }
    })
    fiber.sleep(3)

    local res = wasm.call(m, "foo", "127.1.2.4", 8080)
    t.assert_not(res)
    t.assert(wasm.join(handler))
end

g.test_acl_deny_server_127_1_2_4_8080 = function()
    local out = fio.pathjoin(fio.tempdir(), "out.txt")

    local handler = wasm.run(m, {
        stdout = out,
        args = {"127.1.2.4", "8000"},
        allowed_ips = { "127.0.0.1" },
        allowed_ports = { 8080 },
    })
    t.assert(wasm.join(handler))

    local data = read_file(out)
    t.assert_str_contains(data, "[Errno 2] Permission denied")
end
