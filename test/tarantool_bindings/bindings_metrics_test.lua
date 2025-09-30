local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local test_group = "bindings_metrics_test"

local g = t.group(test_group)

local path =
    fio.pathjoin(utils.get_test_dir(), "tarantool_bindings",
                 "wasm_component_test",  test_group, "python", "adder.wasm")

local function read_file(path)
    local f = assert(io.open(path, "r"))
    local data = f:read("*a")
    f:close()
    return data
end

g.test_encode_decode = function()
    local out = fio.pathjoin(fio.tempdir(), "out.txt")

    local m = wasm.load(path, {
        stdout = out
    })

    local handler = wasm.run(m)
    assert(wasm.join(handler))

    local res = read_file(out)

    local nums = {}
    for n in res:gmatch("[%d%.]+") do
        table.insert(nums, tonumber(n))
    end

    t.assert_equals(#nums, 2)

    local diff = math.abs(nums[2] - nums[1])
    t.assert_almost_equals(diff, 1.0, 0.1)
end
