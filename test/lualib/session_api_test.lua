local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')

package.path = "./test/?.lua;" .. package.path
local utils = require("utils")

local g = t.group("session_api")

local wasm_path = fio.pathjoin(
    utils.get_test_dir(),
    "lualib",
    "wasm_component_test",
    "session_test",
    "adder.wasm"
)

g.before_all(function(cg)   
    cg.module = wasm.load(wasm_path)
end)

g.test_session = function(cg)
    t.assert_equals(wasm.call(cg.module, "t-func"), 1)
    t.assert_equals(wasm.call(cg.module, "t-func"), 1)

    local session = wasm.open_session(cg.module)

    t.assert_equals(wasm.session_call(session, "t-func"), 1)
    t.assert_equals(wasm.session_call(session, "t-func"), 2)
    t.assert_equals(wasm.session_call(session, "t-func"), 3)

    local batch_res = wasm.session_batch(session, {
        {"t-func"},
        {"t-func"},
    })
    t.assert_equals(batch_res, {4, 5})

    t.assert(wasm.close_session(session))
    t.assert_error_msg_contains("not found",
                                wasm.session_call, session, "t-func")

    t.assert_error_msg_contains("not found",
                                wasm.close_session, session, "t-func")

    t.assert_equals(wasm.call(cg.module, "t-func"), 1)

    local new_session = wasm.open_session(cg.module)

    t.assert_equals(wasm.session_call(new_session, "t-func"), 1)
    t.assert(wasm.drop(cg.module))

    t.assert_error_msg_contains("not found",
                                wasm.session_call, session, "t-func")
end
