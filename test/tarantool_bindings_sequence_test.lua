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
    cg.seq = box.schema.sequence.create('test_seq', { if_not_exists = true })
    cg.wasm_seq = { id = cg.seq.id }
end)

g.after_all(function()
    utils.cleanup_logs(fio.cwd())
end)

g.test_sequence_basic = function(cg)
    local res = cg.m:t_current(cg.wasm_seq)
    t.assert_not(res[1])
    t.assert_str_contains(res[2].message, "Sequence 'test_seq' is not started")

    res = cg.m:t_next(cg.wasm_seq)
    t.assert_equals(res, { true, 1 })

    res = cg.m:t_current(cg.wasm_seq)
    t.assert_equals(res, { true, 1 })

    res = cg.m:t_next(cg.wasm_seq)
    t.assert_equals(res, { true, 2 })

    res = cg.m:t_set(cg.wasm_seq, 100)
    t.assert_equals(res, { true })

    res = cg.m:t_current(cg.wasm_seq)
    t.assert_equals(res, { true, 100 })

    res = cg.m:t_next(cg.wasm_seq)
    t.assert_equals(res, { true, 101 })

    res = cg.m:t_next(cg.wasm_seq)
    t.assert_equals(res, { true, 102 })

    res = cg.m:t_reset(cg.wasm_seq)
    t.assert_equals(res, { true })

    res = cg.m:t_current(cg.wasm_seq)
    t.assert_not(res[1])
    t.assert_str_contains(res[2].message, "Sequence 'test_seq' is not started")

    res = cg.m:t_next(cg.wasm_seq)
    t.assert_equals(res, { true, 1 })

    res = cg.m:t_next(cg.wasm_seq)
    t.assert_equals(res, { true, 2 })
end
