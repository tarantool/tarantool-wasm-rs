local t = require('luatest')
local luawasm = require('luawasm')
local fio = require('fio')
local fiber = require('fiber')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local test_group = "bindings_sequence_test"

local g = t.group(
    test_group,
    t.helpers.matrix({
        lang = {"python"}
    })
)

g.after_all(function()
    utils.cleanup_logs(fio.cwd())
end)

g.test_sequence_basic = function(cg)
    t.skip()

    box.cfg{log_level = 7}

    local path = fio.pathjoin(utils.get_test_dir(), "wasm_component_test", test_group, "python")
    local m =  luawasm:new({
        dir = path,
    })
    local seq = box.schema.sequence.create('test_seq', { if_not_exists = true })
    t.assert_equals(seq.id, 1)

    local ok, res = pcall(function()
        m:run()
    end)

    t.assert(ok, res)
    t.assert(m:join())
end
