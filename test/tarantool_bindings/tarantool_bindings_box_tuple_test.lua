local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')
local json = require('json')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local enc = utils.enc
local dec = utils.dec

local test_group = "bindings_box_tuple_test"

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
    cg.s = wasm.open_session(cg.m)

    box.cfg{}
end)

g.after_all(function()
    utils.cleanup_logs(fio.cwd())
end)

g.test_create_tuple_and_refcount = function(cg)
    local data = enc({1, "abc", {x = 42}})
    local res = wasm.session_call(cg.s, "t-new", data)
    t.assert(res[1])
    local tup = res[2]

    res = wasm.session_batch(cg.s, {
        {"t-to-bytes", tup},
        {"t-retain", tup},
        {"t-release", tup},
    })

    t.assert(res[1][1])
    t.assert_equals(dec(res[1][2]), {1, "abc", {x = 42}})
    t.assert_equals(res[2], nil)
    t.assert_equals(res[3], nil)
end

g.test_tuple_fields = function(cg)
    local data = enc({1, "abc", {x = 42}})
    local res = wasm.session_call(cg.s, "t-new", data)
    t.assert(res[1])
    local tup = res[2]

    res = wasm.session_batch(cg.s, {
        {"t-field-count", tup},
        {"t-byte-size", tup},
        {"t-field", tup, 0},
        {"t-field", tup, 1},
        {"t-field", tup, 2},
        {"t-field-by-path", tup, "[2].x"},
    })

    t.assert_equals(res[1], 3)
    t.assert_equals(res[2], 10)

    t.assert(res[3][1])
    t.assert(res[4][1])
    t.assert(res[5][1])
    t.assert(res[6][1])

    t.assert_equals(dec(res[3][2]), 1)
    t.assert_equals(dec(res[4][2]), "abc")
    t.assert_equals(dec(res[5][2]), {x = 42})
    t.assert_equals(dec(res[6][2]), 42)
end

g.test_tuple_format_and_validation = function(cg)
    local tf = wasm.session_call(cg.s, "t-tuple-format-default")

    local tup_data = enc({1, "abc", {x = 42}})
    local res = wasm.session_call(cg.s, "t-new", tup_data)
    t.assert(res[1])
    local tup = res[2]

    local valid = wasm.session_call(cg.s, "t-validate", tup, tf)
    t.assert_equals(valid, {true})

    local tf2 = wasm.session_call(cg.s, "t-format", tup)
    t.assert_type(tf2.handle, "number")
end

g.test_tuple_update = function(cg)
    local src = {1, "abc", {x = 42}}
    local data = enc(src)
    local res = wasm.session_call(cg.s, "t-new", data)
    t.assert(res[1])
    local tup = res[2]

    local update_expr = enc({{"=", 2, "xyz"}})
    res = wasm.session_call(cg.s, "t-update", tup, update_expr)
    t.assert(res[1])
    local updated = res[2]

    res = wasm.session_call(cg.s, "t-to-bytes", updated)
    t.assert(res[1])
    t.assert_equals(dec(res[2]), {1, "xyz", {x = 42}})
end

g.test_tuple_upsert = function(cg)
    local src = {1, "abc", {x = 42}}
    local data = enc(src)
    local res = wasm.session_call(cg.s, "t-new", data)
    t.assert(res[1])
    local tup = res[2]

    local upsert_expr = enc({{"+", 1, 100}})
    res = wasm.session_call(cg.s, "t-upsert", tup, upsert_expr)
    t.assert(res[1])
    local upserted = res[2]

    res = wasm.session_call(cg.s, "t-to-bytes", upserted)
    t.assert(res[1])
    t.assert_equals(dec(res[2]), {101, "abc", {x = 42}})
end

g.test_tuple_compare = function(cg)
    local parts = {{
        ["field-no"] = 1,
        ["field-type"] = "unsigned",
        ["flags"] = {"is-nullable"},
    }}

    local res = wasm.session_call(cg.s, "t-key-def-new", parts)
    t.assert(res[1])
    local kd = res[2]

    local data = enc({1, "abc", {tmp = "value"}})
    res = wasm.session_call(cg.s, "t-new", data)
    t.assert(res[1])
    local tup1 = res[2]
    wasm.session_call(cg.s, "t-retain", tup1)

    res = wasm.session_call(cg.s, "t-new", data)
    t.assert(res[1])
    local tup2 = res[2]
    wasm.session_call(cg.s, "t-retain", tup2)

    local cmp = wasm.session_call(cg.s, "t-compare", kd, tup1, tup2)
    t.assert_equals(cmp, 0)
end

g.test_tuple_compare_with_key = function(cg)
    local parts = {{
        ["field-no"] = 0,
        ["field-type"] = "unsigned",
        ["flags"] = {"is-nullable"},
    }}

    local res = wasm.session_call(cg.s, "t-key-def-new", parts)
    t.assert(res[1])
    local kd = res[2]

    local data = enc({1, "abc", {tmp = "value"}})
    res = wasm.session_call(cg.s, "t-new", data)
    t.assert(res[1])
    local tup = res[2]

    local key = enc({1})
    local cmp_key = wasm.session_call(cg.s, "t-compare-with-key", tup, key, kd)
    t.assert_equals(cmp_key, {true, 0})
end

g.test_tuple_iterator_all = function(cg)
    local src = {1, "abc", {x = 42}}
    local data = enc(src)
    local res = wasm.session_call(cg.s, "t-new", data)
    t.assert(res[1])
    local tup = res[2]

    res = wasm.session_call(cg.s, "t-iter-test-all", tup)

    t.assert(res[1], res)
    local result = dec(res[2])

    t.assert_equals(result[1], {0, 1})
    t.assert_equals(result[2], {1, "abc"})
    t.assert_equals(result[3], {2, {x = 42}})

    t.assert_equals(result[4][1], "rewind")
    t.assert_equals(result[4][2], 0)
    t.assert_equals(result[4][3], 1)

    t.assert_equals(result[5][1], "seek")
    t.assert_equals(result[5][2], 2)
    t.assert_equals(result[5][3], "abc")
end
