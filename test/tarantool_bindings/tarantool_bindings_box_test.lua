local t = require('luatest')
local luawasm = require('luawasm')
local fio = require('fio')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local enc = utils.enc
local dec = utils.dec

local test_group = "bindings_box_test"

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
    box.schema.space.create("tester", { if_not_exists = true })
    box.space.tester:format({ { name = "id", type = "unsigned" }, { name = "value", type = "string" } })
    box.space.tester:create_index("primary", { if_not_exists = true })
    box.space.tester:truncate()

    box.space.tester:insert({ 1, "a" })
    box.space.tester:insert({ 2, "b" })
    box.space.tester:insert({ 3, "c" })

    cg.idx = {
        ["id"] = box.space.tester.index.primary.id,
        ["space-id"] = box.space.tester.id,
        ["index-base"] = 0,
    }
    cg.sp = {
        id = box.space.tester.id
    }
end)

g.after_all(function()
    utils.cleanup_logs(fio.cwd())
end)

-- schema-version
g.test_schema_version = function(cg)
    local ver = cg.m:t_schema_version()
    t.assert_type(ver, "number")
end

-- space-by-name
g.test_space_by_name = function(cg)
    local res = cg.m:t_space_by_name("tester")
    t.assert(res[1])
    local space = res[2]
    t.assert_equals(space, cg.sp)
end

-- index-by-name
g.test_index_by_name = function(cg)
    local res = cg.m:t_index_by_name(cg.sp, "primary")
    t.assert(res[1])
    local index = res[2]
    t.assert_equals(index, cg.idx)
end

g.test_box_ops_scenario = function(cg)
    -- Insert
    local tuple = enc({ 10, "hello" })
    local res = cg.m:t_insert(cg.sp, tuple)
    t.assert(res[1])
    local tup = dec(res[2])
    t.assert_equals(tup, { 10, "hello" })

    -- Replace
    local replaced = enc({ 10, "world" })
    res = cg.m:t_replace(cg.sp, replaced)
    t.assert(res[1])
    tup = dec(res[2])
    t.assert_equals(tup, { 10, "world" })

    -- Update
    local key = enc({ 10 })
    local ops = enc({ { "=", 1, "updated" } })
    res = cg.m:t_update(cg.idx, key, ops)
    t.assert(res[1])
    tup = dec(res[2])
    t.assert_equals(tup, { 10, "updated" })

    -- Upsert
    local upserted = enc({ 20, "new" })
    ops = enc({ { "=", 1, "upserted" } })
    res = cg.m:t_upsert(cg.idx, upserted, ops) -- id=20 does not exist → insert
    t.assert(res[1])
    local found = box.space.tester:get(20)
    t.assert_equals(found, { 20, "new" })

    res = cg.m:t_upsert(cg.idx, upserted, ops) -- id=20 exists →  set field #2 to "upserted"
    t.assert(res[1])
    local found = box.space.tester:get(20)
    t.assert_equals(found, { 20, "upserted" })

    -- Delete
    key = enc({ 10 })
    res = cg.m:t_delete(cg.idx, key)
    t.assert(res[1])
    tup = dec(res[2])
    t.assert_equals(tup, { 10, "updated" })
    t.assert_equals(box.space.tester:get(10), nil)

    -- Truncate
    t.assert(box.space.tester:count() > 0)
    res = cg.m:t_truncate(cg.sp)
    t.assert(res[1])
    t.assert_equals(box.space.tester:count(), 0)
end

g.test_box_ops_negative = function(cg)
    local bad_tuple = enc({ id = 1 })
    local bad_key = enc("not_array")
    local ops = enc({ { "=", 1, "fail" } })

    local fake_space = { id = 9999 }
    local fake_index = {
        ["id"] = 9999,
        ["space-id"] = 9999,
        ["index-base"] = 0,
    }

    -- t_insert with invalid msgpack
    local res = cg.m:t_insert(cg.sp, bad_tuple)
    t.assert_equals(res[1], false)
    t.assert_str_contains(res[2].message, "Expected MsgPack array")

    -- t_replace with invalid data
    res = cg.m:t_replace(cg.sp, bad_tuple)
    t.assert_equals(res[1], false)
    t.assert_str_contains(res[2].message, "Expected MsgPack array")

    -- t_update with invalid key
    res = cg.m:t_update(cg.idx, bad_key, ops)
    t.assert_equals(res[1], false)
    t.assert_str_contains(res[2].message, "Expected MsgPack array")

    -- t_delete with invalid key
    res = cg.m:t_delete(cg.idx, bad_key)
    t.assert_equals(res[1], false)
    t.assert_str_contains(res[2].message, "Expected MsgPack array")

    -- t_upsert with invalid tuple
    res = cg.m:t_upsert(cg.idx, bad_tuple, ops)
    t.assert_equals(res[1], false)
    t.assert_str_contains(res[2].message, "Expected MsgPack array")

    -- t_insert into nonexistent space
    res = cg.m:t_insert(fake_space, enc({ 1, "x" }))
    t.assert_equals(res[1], false)
    t.assert_str_contains(res[2].message, "Space '9999' does not exist")

    -- t_update with nonexistent index
    res = cg.m:t_update(fake_index, enc({ 1 }), ops)
    t.assert_equals(res[1], false)
    t.assert_str_contains(res[2].message, "Space '9999' does not exist")

    -- t_truncate with invalid space
    res = cg.m:t_truncate(fake_space)
    t.assert_equals(res[1], false)
    t.assert_str_contains(res[2].message, "Space '9999' does not exist")
end
