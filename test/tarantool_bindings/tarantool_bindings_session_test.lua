local t = require('luatest')
local luawasm = require('luawasm')
local fio = require('fio')
local fiber = require('fiber')
local net = require('net.box')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local enc = utils.enc
local dec = utils.dec

local test_group = "bindings_session_test"

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

-- current
g.test_current = function(cg)
    local res = cg.m:t_current()
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
    local res = cg.m:t_broadcast("foo", payload)

    t.assert_equals(res, {true})
    
    fiber.sleep(0.01)

    t.assert(called, true)
    t.assert_equals(received_value, {"foo", {a = 42}}) 
end

-- iproto-send
g.test_iproto_send = function(cg)
    t.skip("Skipping: unclear how to use box_iproto_send in current test")
    
    local captured = {}

    box.iproto.override(box.iproto.type.UNKNOWN, function(sid, header, body)
        table.insert(captured, {
            sid = sid,
            header = dec(header),
            body = dec(body),
        })
        box.iproto.send(sid, {
            request_type = box.iproto.type.OK,
            sync = header.SYNC,
            schema_version = box.info.schema_version,
        }, { ok = true })
        return true
    end)

    local header = enc({
        SYNC = 42,
        request_type = 999,
    })
    local body = enc({ message = "hi" })

    local res = cg.m:t_iproto_send({ id = box.session.id() }, header, body)

    t.assert_equals(res, {true})

    fiber.sleep(0.05)

    t.assert_equals(#captured, 1)
    local req = captured[1]
    t.assert_equals(req.header.SYNC, 42)
    t.assert_equals(req.header.request_type, 999)
    t.assert_equals(req.body.message, "hi")
end
