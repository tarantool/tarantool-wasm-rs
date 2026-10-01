local t = require("luatest")
local wasm = require("wasm")
local fio = require("fio")
local socket = require("socket")

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

local function with_tcp_server(host, port, fn)
    local server = assert(socket.tcp_server(host, port, function(client)
        client:close()
    end))
    local ok, result = pcall(fn)
    server:close()
    if not ok then
        error(result, 0)
    end
    return result
end

local function assert_client_allowed(host, port)
    with_tcp_server(host, port, function()
        t.assert(wasm.call(m, "foo", host, port))
    end)
end

local function assert_server_allowed(host, port)
    local handler = wasm.run(m, {
        inherit_stdout = false,
        args = {host, tostring(port)},
    })

    t.helpers.retrying({timeout = 3, delay = 0.01}, function()
        local client = assert(socket.tcp_connect(host, port, 0.1))
        client:close()
    end)
    t.assert(wasm.join(handler))
end

g.test_acl_allow_localhost_8000 = function()
    assert_client_allowed("127.0.0.1", 8000)
    assert_server_allowed("127.0.0.1", 8000)
end

g.test_acl_allow_127_1_2_3_8080 = function()
    t.skip_if(jit.os == "OSX", "macOS requires an explicit loopback alias for 127.1.2.3")
    assert_client_allowed("127.1.2.3", 8080)
    assert_server_allowed("127.1.2.3", 8080)
end

g.test_acl_deny_localhost_1234 = function()
    with_tcp_server("127.0.0.1", 1234, function()
        t.assert_not(wasm.call(m, "foo", "127.0.0.1", 1234))
    end)
end

g.test_acl_deny_127_1_2_4_8080 = function()
    t.skip_if(jit.os == "OSX", "macOS requires an explicit loopback alias for 127.1.2.4")
    with_tcp_server("127.1.2.4", 8080, function()
        t.assert_not(wasm.call(m, "foo", "127.1.2.4", 8080))
    end)
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
