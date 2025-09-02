local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local test_group = "bindings_sequence_test"

local g = t.group(
    test_group,
    t.helpers.matrix({
        lang = {"python", "c", "js", "go"}
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
end)

g.before_each(function(cg)
    cg.seq = box.schema.sequence.create('test_seq', {
        if_not_exists = true
    })
    cg.wasm_seq = {
        id = cg.seq.id
    }
end)

g.after_each(function(cg)
    cg.seq:drop()
    cg.wasm_seq = nil
end)

g.after_all(function()
    utils.cleanup_logs(fio.cwd())
end)

g.test_sequence_basic = function(cg)
    local res = wasm.call(cg.m, "t-current", cg.wasm_seq)
    t.assert_not(res[1])
    t.assert_str_contains(
        res[2].message,
        "Sequence 'test_seq' is not started"
    )

    res = wasm.call(cg.m, "t-next", cg.wasm_seq)
    t.assert_equals(res, {true, 1})

    res = wasm.call(cg.m, "t-current", cg.wasm_seq)
    t.assert_equals(res, {true, 1})

    res = wasm.call(cg.m, "t-next", cg.wasm_seq)
    t.assert_equals(res, {true, 2})

    res = wasm.call(cg.m, "t-set", cg.wasm_seq, 100)
    t.assert_equals(res, {true})

    res = wasm.call(cg.m, "t-current", cg.wasm_seq)
    t.assert_equals(res, {true, 100})

    res = wasm.call(cg.m, "t-next", cg.wasm_seq)
    t.assert_equals(res, {true, 101})

    res = wasm.call(cg.m, "t-next", cg.wasm_seq)
    t.assert_equals(res, {true, 102})

    res = wasm.call(cg.m, "t-reset", cg.wasm_seq)
    t.assert_equals(res, {true})

    res = wasm.call(cg.m, "t-current", cg.wasm_seq)
    t.assert_not(res[1])
    t.assert_str_contains(
        res[2].message,
        "Sequence 'test_seq' is not started"
    )

    res = wasm.call(cg.m, "t-next", cg.wasm_seq)
    t.assert_equals(res, {true, 1})

    res = wasm.call(cg.m, "t-next", cg.wasm_seq)
    t.assert_equals(res, {true, 2})
end

