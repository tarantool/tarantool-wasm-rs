local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local test_group = "bindings_tuple_format_test"

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
end)

g.after_all(function()
    utils.cleanup_logs(fio.cwd())
end)

g.test_tuple_format_lifecycle = function(cg)
    local parts = {
        {
            ["field-no"] = 1,
            ["field-type"] = "unsigned",
            ["flags"] = {"is-nullable"},
        },
        {
            ["field-no"] = 2,
            ["field-type"] = "string",
            ["collation"] = "unicode",
            ["path"] = "tmp",
            ["flags"] = {"exclude-null"},
        },
    }

    local res = wasm.call(cg.m, "t-key-def-new", parts)
    t.assert(res[1])
    local kd = res[2]
    t.assert_type(kd, "table")
    t.assert_type(kd.ptr, "number")

    res = wasm.call(cg.m, "t-new", {kd})
    t.assert(res[1])
    local tf = res[2]
    t.assert_type(tf, "table")
    t.assert_type(tf.ptr, "number")

    -- Increase refcount
    wasm.call(cg.m, "t-ref", tf)

    -- Decrease refcount
    wasm.call(cg.m, "t-unref", tf)

    -- Get default tuple_format
    local default_tf = wasm.call(cg.m, "t-default")
    t.assert_type(default_tf, "table")
    t.assert_type(default_tf.ptr, "number")
end
