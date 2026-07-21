local t = require("luatest")
local wasm = require("wasm")
local fio = require("fio")
local socket = require("socket")

package.path = "./test/?.lua;" .. package.path
local utils = require("utils")

local g = t.group("tls_test")

local tls_component = fio.pathjoin(
    utils.get_test_dir(),
    "lualib",
    "wasm_component_test",
    "tls_test",
    "adder.wasm"
)

g.before_all(function(cg)
    cg.tls_module = wasm.load(tls_component)
end)

g.after_all(function(cg)
    if cg.tls_module ~= nil then
        pcall(wasm.drop, cg.tls_module)
    end
end)

local function run_plain_tcp_server(port)
    return assert(socket.tcp_server("127.0.0.1", port, function(client)
        client:close()
    end))
end

g.test_tls_handshake_fails_on_plain_tcp = function(cg)
    local ports = { 18443, 19443, 20443, 21443, 22443 }
    local last_result = nil
    local got_tls_error = false

    for _, port in ipairs(ports) do
        local server = run_plain_tcp_server(port)

        local ok, result = pcall(
            wasm.call,
            cg.tls_module,
            "t-tls-handshake-plain-fails",
            "127.0.0.1",
            port
        )
        last_result = result
        server:close()
        if not ok then
            error(result, 0)
        end

        if type(result) == "string" then
            if string.find(result, "tls-handshake-error:", 1, true) ~= nil then
                got_tls_error = true
                break
            end
        end
    end

    t.assert_equals(
        got_tls_error,
        true,
        ("expected tls handshake error, got: %s"):format(tostring(last_result))
    )
end


local function resolve_ipv4(host)
    local ok, addrs = pcall(socket.getaddrinfo, host)
    if not ok or addrs == nil then
        return nil
    end

    for _, addr in ipairs(addrs) do
        if addr.family == "AF_INET" and addr.host ~= nil then
            return addr.host
        end
    end

    return nil
end


g.test_tls_handshake_success_on_public_tls = function(cg)
    local targets = {
        { host = "example.com", port = 443, server_name = "example.com" },
        { host = "www.cloudflare.com", port = 443, server_name = "www.cloudflare.com" },
        { host = "www.google.com", port = 443, server_name = "www.google.com" },
    }

    local last_result = nil
    local success = false

    for _, target in ipairs(targets) do
        local ip = resolve_ipv4(target.host)
        if ip ~= nil then
            local result = wasm.call(
                cg.tls_module,
                "t-tls-handshake",
                ip,
                target.port,
                target.server_name
            )
            last_result = result

            if result == "tls-handshake-success" then
                success = true
                break
            end
        end
    end

    t.assert_equals(
        success,
        true,
        ("expected tls-handshake-success, got: %s"):format(tostring(last_result))
    )
end
