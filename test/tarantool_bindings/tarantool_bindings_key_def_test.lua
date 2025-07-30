local t = require('luatest')
local luawasm = require('luawasm')
local fio = require('fio')
local msgpack = require("msgpack")

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local enc = utils.enc
local dec = utils.dec

local test_group = "bindings_key_def_test"

local g = t.group(
    test_group,
    t.helpers.matrix({
        lang = {"python", "c", "js", "go"}
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

local function assert_part(actual, expected)
    t.assert_equals(actual["field-no"], expected["field-no"])
    t.assert_equals(actual["field-type"], expected["field-type"])
    t.assert_equals(actual["collation"], expected["collation"])
    t.assert_equals(actual["path"], expected["path"])
    t.assert_equals(actual["flags"], expected["flags"])
end

local key_parts_1 =  {
    {
        ["field-no"] = 0,
        ["field-type"] = "unsigned",
        ["flags"] = { "is-nullable" },
    },
}

local key_parts_2 = {
    {
        ["field-no"] = 1,
        ["field-type"] = "unsigned",
        ["flags"] = { "is-nullable" },
    },
    {
        ["field-no"] = 2,
        ["field-type"] = "string",
        ["collation"] = "unicode",
        ["path"] = "tmp",
        ["flags"] = { "exclude-null" },
    },
}

local key_parts_3 = {
    { ["field-no"] = 4, ["field-type"] = "unsigned", ["flags"] = { "is-nullable" } },
    { ["field-no"] = 2, ["field-type"] = "string", ["flags"] = { "is-nullable" } },
}

-- new
g.test_new = function(cg)
    local key_parts = key_parts_1

    local res = cg.m:t_new(key_parts)

    t.assert(res[1])

    local ptr = res[2]
    t.assert_type(ptr, "number")

    res = cg.m:t_dump_parts(ptr)
    t.assert(res[1])

    local parts = res[2]
    t.assert_type(parts, "table")
    t.assert_equals(#parts, 1)
    assert_part(parts[1], key_parts[1])

    cg.m:t_delete(ptr)
end

-- dump_parts
g.test_dump_parts = function(cg)
    local key_parts = key_parts_2

    local res = cg.m:t_new(key_parts)
    t.assert(res[1], res[2])
    local ptr = res[2]

    res = cg.m:t_dump_parts(ptr)
    t.assert(res[1])
    local parts = res[2]

    t.assert_equals(#parts, 2)
    assert_part(parts[1], key_parts[1])
    assert_part(parts[2], key_parts[2])

    cg.m:t_delete(ptr)
end

-- dup
g.test_dup = function(cg)
    local key_parts = key_parts_2

    local res = cg.m:t_new(key_parts)
    t.assert(res[1])
    local orig_ptr = res[2]

    local dup_res = cg.m:t_dup(orig_ptr)
    t.assert(dup_res[1])
    local dup_ptr = dup_res[2]

    t.assert_not_equals(orig_ptr, dup_ptr)

    local dump_res = cg.m:t_dump_parts(dup_ptr)
    t.assert(dump_res[1])
    local parts = dump_res[2]
    t.assert_equals(#parts, 2)

    assert_part(parts[1], key_parts[1])
    assert_part(parts[2], key_parts[2])

    cg.m:t_delete(orig_ptr)
    cg.m:t_delete(dup_ptr)
end

-- merge
g.test_merge = function(cg)
    local key_parts_1 = key_parts_1
    local key_parts_2 = key_parts_2

    local res1 = cg.m:t_new(key_parts_1)
    t.assert(res1[1])
    local ptr1 = res1[2]

    local res2 = cg.m:t_new(key_parts_2)
    t.assert(res2[1])
    local ptr2 = res2[2]

    local merge_res = cg.m:t_merge(ptr1, ptr2)
    t.assert(merge_res[1])
    local merged_ptr = merge_res[2]

    local dump_res = cg.m:t_dump_parts(merged_ptr)
    t.assert(dump_res[1])
    local parts = dump_res[2]
    t.assert_equals(#parts, 3)

    assert_part(parts[1], key_parts_1[1])
    assert_part(parts[2], key_parts_2[1])
    assert_part(parts[3], key_parts_2[2])

    cg.m:t_delete(ptr1)
    cg.m:t_delete(ptr2)
    cg.m:t_delete(merged_ptr)
end

-- validate-key
g.test_validate_key = function(cg)
    local key_def = cg.m:t_new(key_parts_3)
    t.assert(key_def[1])
    local ptr = key_def[2]

    local res = cg.m:t_validate_key(ptr, enc({'a', 'b'}))
    t.assert_not(res[1])
    t.assert_str_contains(res[2].message, "expected unsigned")

    res = cg.m:t_validate_key(ptr, enc({1, 2}))
    t.assert_not(res[1])
    t.assert_str_contains(res[2].message, "expected string")

    res = cg.m:t_validate_key(ptr, enc({1, 'a', 1}))
    t.assert_not(res[1])
    t.assert_str_contains(res[2].message, "Invalid key part count")

    res = cg.m:t_validate_key(ptr, enc({}))
    t.assert_equals(res, { true, { true, 1 } })

    res = cg.m:t_validate_key(ptr, enc({1}))
    t.assert_equals(res, { true, { true, 2 } })

    res = cg.m:t_validate_key(ptr, enc({1, 'a'}))
    t.assert_equals(res, { true, { true, 4 } })

    cg.m:t_delete(ptr)
end

-- validate-full-key
g.test_validate_full_key = function(cg)
    local key_def = cg.m:t_new(key_parts_3)
    t.assert(key_def[1])
    local ptr = key_def[2]

    local res = cg.m:t_validate_full_key(ptr, enc({'a', 'b'}))
    t.assert_not(res[1])
    t.assert_str_contains(res[2].message, "expected unsigned")

    res = cg.m:t_validate_full_key(ptr, enc({1, 2}))
    t.assert_not(res[1])
    t.assert_str_contains(res[2].message, "expected string")

    res = cg.m:t_validate_full_key(ptr, enc({1}))
    t.assert_not(res[1])
    t.assert_str_contains(res[2].message, "expected 2, got 1")

    res = cg.m:t_validate_full_key(ptr, enc({1, 'a', 1}))
    t.assert_not(res[1])
    t.assert_str_contains(res[2].message, "expected 2, got 3")

    res = cg.m:t_validate_full_key(ptr, enc({1, 'a'}))
    t.assert_equals(res, { true, { true, 4 } })

    cg.m:t_delete(ptr)
end

-- validate-tuple
g.test_validate_tuple = function(cg)
    local key_def = cg.m:t_new(key_parts_3)
    t.assert(key_def[1])
    local ptr = key_def[2]

    local tuple, res

    res = cg.m:t_tuple_new(enc({nil, nil, 'a', nil, 'b'}))
    t.assert(res[1])
    tuple = res[2]
    t.assert_equals(cg.m:t_validate_tuple(ptr, tuple), false)

    res = cg.m:t_tuple_new(enc({nil, nil, 1, nil, 2}))
    t.assert(res[1])
    tuple = res[2]
    t.assert_equals(cg.m:t_validate_tuple(ptr, tuple), false)

    res = cg.m:t_tuple_new(enc({1, 2}))
    t.assert(res[1])
    tuple = res[2]
    t.assert_equals(cg.m:t_validate_tuple(ptr, tuple), true)

    res = cg.m:t_tuple_new(enc({nil, nil, 'a', nil, 1}))
    t.assert(res[1])
    tuple = res[2]
    t.assert_equals(cg.m:t_validate_tuple(ptr, tuple), true)

    cg.m:t_delete(ptr)
end

-- part-count
g.test_part_count = function(cg)
    local key_def = cg.m:t_new(key_parts_3)
    t.assert(key_def[1])
    local ptr = key_def[2]

    local res = cg.m:t_part_count(ptr)
    t.assert(res[1])
    t.assert_equals(res[2], 2)
end

-- extract-key
g.test_extract_key = function(cg)
    local key_def = cg.m:t_new(key_parts_3)
    t.assert(key_def[1])
    local ptr = key_def[2]

    local tuple_res = cg.m:t_tuple_new(enc({nil, nil, 'a', nil, 123}))
    t.assert(tuple_res[1])
    local tuple = tuple_res[2]

    local res = cg.m:t_extract_key(ptr, tuple)
    t.assert(res[1])

    local raw_key = res[2]
    t.assert_type(raw_key, "table")

    local decoded = dec(raw_key)
    t.assert_equals(decoded, {123, 'a'})

    cg.m:t_delete(ptr)
end
