local t = require('luatest')
local luawasm = require('luawasm')
local fio = require('fio')
local json = require('json')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local enc = utils.enc
local dec = utils.dec
local dec_ptr = utils.dec_ptr

local test_group = "bindings_box_tuple_test"

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

g.test_create_tuple_and_refcount = function(cg)
    local data = enc({ 1, "abc", { x = 42 } })
    local res = cg.m:t_new(data)
    t.assert(res[1])
    local tup = res[2]
    res = cg.m:batch{
        {cg.m.t_to_buf, tup},
        {cg.m.t_ref, tup},
        {cg.m.t_unref, tup},
    }

    t.assert(res[1][1])
    t.assert_equals(dec(res[1][2]), { 1, "abc", { x = 42 } })
    t.assert_equals(res[2], 0)
    t.assert_equals(res[3], nil)
end

g.test_tuple_fields = function(cg)
    local data = enc({ 1, "abc", { x = 42 } })

    local res = cg.m:t_new(data)
    t.assert(res[1])
    local tup = res[2]

    res = cg.m:batch{
        {cg.m.t_field_count, tup},
        {cg.m.t_bsize, tup},
        {cg.m.t_field, tup, 0},
        {cg.m.t_field, tup, 1},
        {cg.m.t_field, tup, 2},
        {cg.m.t_field_by_path, tup, "[3].x", 1},
    }

    t.assert_equals(res[1], 3)
    t.assert_equals(res[2], 10)

    t.assert(res[3][1])
    t.assert(res[4][1])
    t.assert(res[5][1])
    t.assert(res[6][1])

    t.assert_equals(utils.dec_ptr(res[3][2].ptr), 1)
    t.assert_equals(utils.dec_ptr(res[4][2].ptr), "abc")
    t.assert_equals(utils.dec_ptr(res[5][2].ptr), { x = 42 })
    t.assert_equals(utils.dec_ptr(res[6][2].ptr), 42)
end

g.test_tuple_format_and_validation = function(cg)
    local tf = cg.m:t_tuple_format_default()

    local tup_data = enc({ 1, "abc", { x = 42 } })
    local res = cg.m:t_new(tup_data)
    t.assert(res[1])
    local tup = res[2]

    local valid = cg.m:t_validate(tup, tf)
    t.assert_equals(valid, true)

    local tf2 = cg.m:t_format(tup)
    t.assert_equals(tf, tf2)
end

g.test_tuple_update = function(cg)
    local src = { 1, "abc", { x = 42 } }
    local data = enc(src)

    local res = cg.m:t_new(data)
    t.assert(res[1])
    local tup = res[2]

    local update_expr = enc({ { "=", 2, "xyz" } })

    res = cg.m:t_update(tup, update_expr)
    t.assert(res[1])
    local updated = res[2]

    res = cg.m:t_to_buf(updated)
    t.assert(res[1])
    t.assert_equals(dec(res[2]), { 1, "xyz", { x = 42 } })
end

g.test_tuple_upsert = function(cg)
    local src = { 1, "abc", { x = 42 } }
    local data = enc(src)

    local res = cg.m:t_new(data)
    t.assert(res[1])
    local tup = res[2]

    local upsert_expr = enc({ { "+", 1, 100 } })

    res = cg.m:t_upsert(tup, upsert_expr)
    t.assert(res[1])
    local upserted = res[2]

    res = cg.m:t_to_buf(upserted)
    t.assert(res[1])
    t.assert_equals(dec(res[2]), { 101, "abc", { x = 42 } }) -- 1 + 100 = 101
end

g.test_tuple_compare = function(cg)
    local parts = {
        {
            ["field-no"] = 1,
            ["field-type"] = "unsigned",
            ["flags"] = { "is-nullable" },
        },
    }

    local res = cg.m:t_key_def_new(parts)
    t.assert(res[1])
    local kd = res[2]

    local data = enc({ 1, "abc", { tmp = "value" } })

    res = cg.m:t_new(data)
    t.assert(res[1])
    local tup1 = res[2]
    cg.m:t_ref(tup1)

    res = cg.m:t_new(data)
    t.assert(res[1])
    local tup2 = res[2]
    cg.m:t_ref(tup2)

    local cmp = cg.m:t_compare(kd, tup1, tup2)
    t.assert_equals(cmp, 0)
end

g.test_tuple_compare_with_key = function(cg)
    local parts = {
        {
            ["field-no"] = 0,
            ["field-type"] = "unsigned",
            ["flags"] = { "is-nullable" },
        },
    }

    local res = cg.m:t_key_def_new(parts)
    t.assert(res[1])
    local kd = res[2]

    local data = enc({ 1, "abc", { tmp = "value" } })
    res = cg.m:t_new(data)

    t.assert(res[1])
    local tup = res[2]

    local key = enc({ 1 })
    local cmp_key = cg.m:t_compare_with_key(tup, key, kd)
    t.assert_equals(cmp_key, 0)
end

g.test_tuple_iterator_all = function(cg)
    local src = { 1, "abc", { x = 42 } }
    local data = enc(src)

    local res = cg.m:t_new(data)
    t.assert(res[1])
    local tup = res[2]

    res = cg.m:t_iter_test_all(tup)
    
    t.assert(res[1], res)
    local result = dec(res[2])

    local _dec_ptr = dec_ptr
    if cg.params.lang == "js" then
        -- Workaround for JavaScript: JS cannot represent 64-bit integers directly (e.g. pointer values),
        -- so they are serialized as strings. In this case, we convert them back to 64-bit numbers
        -- using `tonumber64` before passing to `dec_ptr`. For other languages, use `dec_ptr` as-is.
        _dec_ptr = function(p)
            return dec_ptr(tonumber64(p))
        end
    end

    t.assert_equals({ result[1][1], _dec_ptr(result[1][2]) }, { 0, 1 })
    t.assert_equals({ result[2][1], _dec_ptr(result[2][2]) }, { 1, "abc" })
    t.assert_equals({ result[3][1], _dec_ptr(result[3][2]) }, { 2, { x = 42 } })

    t.assert_equals(result[4][1], "rewind")
    t.assert_equals(result[4][2], 0)
    t.assert_equals(_dec_ptr(result[4][3]), 1)

    t.assert_equals(result[5][1], "seek")
    t.assert_equals(result[5][2], 2)
    t.assert_equals(_dec_ptr(result[5][3]), "abc")
end
