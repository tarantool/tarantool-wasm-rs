local t = require('luatest')
local luawasm = require('luawasm')
local fio = require('fio')
local json = require('json')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local dec = utils.dec
local enc = utils.enc

local test_group = "bindings_msgpack_test"

local g = t.group(
    test_group,
    t.helpers.matrix({
        data = {{1, 2, 3}, {a = 1, b = 2}, {1, {a = 2}}}
    })
)

local path = fio.pathjoin(utils.get_test_dir(), "tarantool_bindings", "wasm_component_test",  test_group, "python")
local m = luawasm:new({ dir = path })

g.test_encode_decode = function(cg)
    local tup = cg.params.data

    local expected = enc(tup)

    local json_str = json.encode(tup)
    local json_bytes = { string.byte(json_str, 1, #json_str) }
    local res = m:t_encode(json_bytes)
    t.assert(res[1])
    local actual = res[2]
    t.assert_equals(actual, expected)

    res = m:t_decode(expected)
    t.assert(res[1])
    local decoded_bytes = res[2]
    json_str = string.char(unpack(decoded_bytes))
    t.assert_equals(json.decode(json_str), tup)
end
