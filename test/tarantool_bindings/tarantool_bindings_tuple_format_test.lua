local t = require('luatest')
local luawasm = require('luawasm')
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
    local path = fio.pathjoin(utils.get_test_dir(), "tarantool_bindings", "wasm_component_test",  test_group, cg.params.lang)
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

g.test_tuple_format_lifecycle = function(cg)

    local parts = {
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

    local res = cg.m:t_key_def_new(parts)
    t.assert(res[1])
    local kd = res[2]
    t.assert_type(kd, "table")
    t.assert_type(kd.ptr, "number")

    res = cg.m:t_new({ kd })
    t.assert(res[1])
    local tf = res[2]
    t.assert_type(tf, "table")
    t.assert_type(tf.ptr, "number")

    -- Increase refcount
    cg.m:t_ref(tf)

    -- Decrease refcount
    cg.m:t_unref(tf)

    -- Get default tuple_format
    local default_tf = cg.m:t_default()
    t.assert_type(default_tf, "table")
    t.assert_type(default_tf.ptr, "number")
end

