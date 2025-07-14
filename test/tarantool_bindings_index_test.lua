local t = require('luatest')
local luawasm = require('luawasm')
local fio = require('fio')
local msgpack = require("msgpack")

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local test_group = "bindings_index_test"

local g = t.group(
    test_group,
    t.helpers.matrix({
        lang = {"python", "go", "js", "c"}
    })
)

g.before_all(function(cg)
    local path = fio.pathjoin(utils.get_test_dir(), "wasm_component_test", test_group, cg.params.lang)
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
        ["index-base"] = 1,
    }
end)

g.after_all(function()
    utils.cleanup_logs(fio.cwd())
end)

-- len
g.test_len = function(cg)
    local idx = cg.idx
    local res = cg.m:t_len(idx)
    t.assert(res[1])
    t.assert_equals(res[2], 3)
end

-- bsize
g.test_bsize = function(cg)
    local idx = cg.idx
    local res = cg.m:t_bsize(idx)
    t.assert(res[1])
    t.assert_gt(res[2], 0)
end

-- random
g.test_random = function(cg)
    local idx = cg.idx
    local res = cg.m:t_random(idx, 42)

    t.assert(res[1])
    local msgpack_bytes = res[2]
    t.assert_type(msgpack_bytes, "table")

    local str = string.char(unpack(msgpack_bytes))
    local decoded = msgpack.decode(str)

    t.assert_items_include({ 1, 2, 3 }, { decoded[1] })
    t.assert_items_include({ "a", "b", "c" }, { decoded[2] })
end

-- get
g.test_get = function(cg)
    local idx = cg.idx
    local key = msgpack.encode({ 2 })
    local byte_array = { string.byte(key, 1, #key) }
    local res = cg.m:t_get(idx, byte_array)
    
    t.assert(res[1])
    
    local msgpack_bytes = res[2]
    t.assert_type(msgpack_bytes, "table")

    local str = string.char(unpack(msgpack_bytes))
    local decoded = msgpack.decode(str)

    t.assert_equals(decoded, { 2, "b" })
end

-- min
g.test_min = function(cg)
    local idx = cg.idx
    local key = { string.byte(msgpack.encode({}), 1, -1) }
    local res = cg.m:t_min(idx, key)

    t.assert(res[1])
    local msgpack_bytes = res[2]
    local str = string.char(unpack(msgpack_bytes))
    local decoded = msgpack.decode(str)

    t.assert_equals(decoded, { 1, "a" })
end

-- max
g.test_max = function(cg)
    local idx = cg.idx
    local key = { string.byte(msgpack.encode({}), 1, -1) }
    local res = cg.m:t_max(idx, key)

    t.assert(res[1])
    local msgpack_bytes = res[2]
    local str = string.char(unpack(msgpack_bytes))
    local decoded = msgpack.decode(str)

    t.assert_equals(decoded, { 3, "c" })
end

-- count
g.test_count = function(cg)
    local idx = cg.idx
    local key = { string.byte(msgpack.encode({}), 1, -1) }
    local iter_type = "iter-all"
    local res = cg.m:t_count(idx, iter_type, key)

    t.assert(res[1])
    t.assert_equals(res[2], 3)
end
