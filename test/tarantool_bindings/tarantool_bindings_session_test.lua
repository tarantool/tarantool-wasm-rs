local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')
local msgpack = require('msgpack')
local socket = require('socket')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local enc = utils.enc

local test_group = "bindings_session_test"

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
    if box.cfg.listen == nil then
        box.cfg{listen = fio.pathjoin(fio.cwd(), test_group .. '.sock')}
    end
end)

g.after_all(function()
    utils.cleanup_logs(fio.cwd())
end)

-- current
g.test_current = function(cg)
    local res = wasm.call(cg.m, "t-current")
    t.assert_type(res.id, "number")
end

-- broadcast
g.test_broadcast = function(cg)
    local called = false
    local received_value

    local function my_callback(value, d)
        called = true
        received_value = {value, d}
    end

    box.watch("foo", my_callback)

    local payload = "{a = 42}"
    local res = wasm.call(cg.m, "t-broadcast", "foo", payload)

    t.assert_equals(res, {true})

    t.helpers.retrying({timeout = 2, delay = 0.01}, function()
        t.assert(called)
    end)

    t.assert(called, true)
    t.assert_equals(received_value, {"foo", {a = 42}})
end

-- iproto-send
g.test_iproto_send = function(cg)
    local sid
    local function on_connect()
        sid = box.session.id()
    end
    box.session.on_connect(on_connect)
    local client = socket.tcp_connect('unix/', box.cfg.listen)
    t.assert_is_not(client, nil)
    t.assert_equals(#client:read(128), 128)
    box.session.on_connect(nil, on_connect)
    t.assert_type(sid, 'number')

    local expected_header = {
        SYNC = 42,
        request_type = 999,
    }
    local expected_body = {message = 'hi'}
    local header = enc(expected_header)
    local body = enc(expected_body)

    local res = wasm.call(
        cg.m, 't-iproto-send', {id = sid}, header, body
    )
    t.assert_equals(res, {true})

    local header_raw = string.char(unpack(header))
    local body_raw = string.char(unpack(body))
    local packet = client:read(5 + #header_raw + #body_raw)
    client:close()
    t.assert_equals(#packet, 5 + #header_raw + #body_raw)

    local packet_len, next_offset = msgpack.decode(packet)
    t.assert_equals(#packet - next_offset + 1, packet_len)
    local packet_header
    packet_header, next_offset = msgpack.decode(packet, next_offset)
    local packet_body
    packet_body, next_offset = msgpack.decode(packet, next_offset)
    t.assert_equals(next_offset, #packet + 1)
    t.assert_equals(packet_header, expected_header)
    t.assert_equals(packet_body, expected_body)
end
