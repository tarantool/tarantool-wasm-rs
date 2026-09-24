local t = require('luatest')
local wasm = require('wasm')
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
    local ver = wasm.call(cg.m, "t-schema-version")
    t.assert_type(ver, "number")
end

-- space-by-name
g.test_space_by_name = function(cg)
    local res = wasm.call(cg.m, "t-space-by-name", "tester")
    t.assert(res[1])
    local space = res[2]
    t.assert_equals(space, cg.sp)
end

g.test_space_by_name_missing = function(cg)
    local res = wasm.call(cg.m, "t-space-by-name", "space_that_does_not_exist")
    t.assert_equals(res[1], true)
    t.assert_equals(res[2], nil)
end

-- index-by-name
g.test_index_by_name = function(cg)
    local res = wasm.call(cg.m, "t-index-by-name", cg.sp, "primary")
    t.assert(res[1])
    local index = res[2]
    t.assert_equals(index, cg.idx)
end

g.test_index_by_name_missing = function(cg)
    local res = wasm.call(cg.m, "t-index-by-name", cg.sp, "index_that_does_not_exist")
    t.assert_equals(res[1], true)
    t.assert_equals(res[2], nil)
end

g.test_box_ops_scenario = function(cg)
    -- Insert
    local tuple = enc({10, "hello"})
    local res = wasm.call(cg.m, "t-insert", cg.sp, tuple)
    t.assert(res[1])
    local tup = dec(res[2])
    t.assert_equals(tup, {10, "hello"})

    -- Replace
    local replaced = enc({10, "world"})
    res = wasm.call(cg.m, "t-replace", cg.sp, replaced)
    t.assert(res[1])
    tup = dec(res[2])
    t.assert_equals(tup, {10, "world"})

    -- Update
    local key = enc({10})
    local ops = enc({{"=", 1, "updated"}})
    res = wasm.call(cg.m, "t-update", cg.idx, key, ops)
    t.assert(res[1])
    tup = dec(res[2])
    t.assert_equals(tup, {10, "updated"})

    -- Upsert (id=20 does not exist → insert)
    local upserted = enc({20, "new"})
    ops = enc({{"=", 1, "upserted"}})
    res = wasm.call(cg.m, "t-upsert", cg.idx, upserted, ops)
    t.assert(res[1])
    local found = box.space.tester:get(20)
    t.assert_equals(found, {20, "new"})

    -- Upsert (id=20 exists → update field #2)
    res = wasm.call(cg.m, "t-upsert", cg.idx, upserted, ops)
    t.assert(res[1])
    found = box.space.tester:get(20)
    t.assert_equals(found, {20, "upserted"})

    -- Delete
    key = enc({10})
    res = wasm.call(cg.m, "t-delete", cg.idx, key)
    t.assert(res[1])
    tup = dec(res[2])
    t.assert_equals(tup, {10, "updated"})
    t.assert_equals(box.space.tester:get(10), nil)

    -- Truncate
    t.assert(box.space.tester:count() > 0)
    res = wasm.call(cg.m, "t-truncate", cg.sp)
    t.assert(res[1])
    t.assert_equals(box.space.tester:count(), 0)
end

g.test_box_ops_negative = function(cg)
    local bad_tuple = enc({id = 1})
    local bad_key = enc("not_array")
    local ops = enc({{"=", 1, "fail"}})

    local fake_space = {id = 9999}
    local fake_index = {
        ["id"] = 9999,
        ["space-id"] = 9999,
    }

    -- t-insert with invalid msgpack
    local res = wasm.call(cg.m, "t-insert", cg.sp, bad_tuple)
    t.assert_equals(res[1], false)
    t.assert_str_contains(res[2].message, "Expected MsgPack array")

    -- t-replace with invalid data
    res = wasm.call(cg.m, "t-replace", cg.sp, bad_tuple)
    t.assert_equals(res[1], false)
    t.assert_str_contains(res[2].message, "Expected MsgPack array")

    -- t-update with invalid key
    res = wasm.call(cg.m, "t-update", cg.idx, bad_key, ops)
    t.assert_equals(res[1], false)
    t.assert_str_contains(res[2].message, "Expected MsgPack array")

    -- t-delete with invalid key
    res = wasm.call(cg.m, "t-delete", cg.idx, bad_key)
    t.assert_equals(res[1], false)
    t.assert_str_contains(res[2].message, "Expected MsgPack array")

    -- t-upsert with invalid tuple
    res = wasm.call(cg.m, "t-upsert", cg.idx, bad_tuple, ops)
    t.assert_equals(res[1], false)
    t.assert_str_contains(res[2].message, "Expected MsgPack array")

    -- t-insert into nonexistent space
    res = wasm.call(cg.m, "t-insert", fake_space, enc({1, "x"}))
    t.assert_equals(res[1], false)
    t.assert_str_contains(
        res[2].message,
        "Space '9999' does not exist"
    )

    -- t-update with nonexistent index
    res = wasm.call(cg.m, "t-update", fake_index, enc({1}), ops)
    t.assert_equals(res[1], false)
    t.assert_str_contains(
        res[2].message,
        "Space '9999' does not exist"
    )

    -- t-truncate with invalid space
    res = wasm.call(cg.m, "t-truncate", fake_space)
    t.assert_equals(res[1], false)
    t.assert_str_contains(
        res[2].message,
        "Space '9999' does not exist"
    )
end
