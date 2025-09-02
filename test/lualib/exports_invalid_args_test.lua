local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local test_group = "export_test"

local g = t.group(
    "export_invalid_args_test",
    t.helpers.matrix({
        lang = {"python", "go", "c", "js"}
    })
)

g.before_all(function(cg)
    local path =
        fio.pathjoin(utils.get_test_dir(), "lualib",
                     "wasm_component_test", test_group, cg.params.lang)
    if cg.params.lang == "c" then
        path = fio.pathjoin(path, "adder.component.wasm")
    else
        path = fio.pathjoin(path, "adder.wasm")
    end
    cg.m = wasm.load(path)
end)

-- boolean
g.test_bool_invalid_arg = function(cg)
    -- Any non-boolean input (including strings, numbers, etc.) is
    -- interpreted as `false`. Only `true` is preserved as `true`; all
    -- other values convert to `false`.
    local ok, res = pcall(function()
        return wasm.call(cg.m, "t-bool", "bool")
    end)
    t.assert_equals(ok, true)
    t.assert_equals(res, false)
end

-- signed integers
g.test_s8_invalid_arg = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-s8", "x")
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected s8")
end

g.test_s16_invalid_arg = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-s16", "x")
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected s16")
end

g.test_s32_invalid_arg = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-s32", "x")
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected s32")
end

g.test_s64_invalid_arg = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-s64", "x")
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected s64")
end

-- unsigned integers
g.test_u8_invalid_arg = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-u8", "not a number")
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected u8")
end

g.test_u16_invalid_arg = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-u16", "not a number")
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected u16")
end

g.test_u32_invalid_arg = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-u32", "not a number")
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected u32")
end

g.test_u64_invalid_arg = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-u64", "not a number")
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected u64")
end

-- floats
g.test_f32_invalid_arg = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-f32", {})
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected f32")
end

g.test_f64_invalid_arg = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-f64", {})
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected f64")
end

-- char
g.test_char_invalid_arg = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-char", "abc")
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(
        tostring(err),
        "expected single character string"
    )
end

-- string
g.test_string_invalid_arg = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-string", 123)
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected string")
end

-- list<string>
g.test_list_invalid_arg = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-list", "not a list")
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected array for list")
end

-- tuple 
g.test_tuple_invalid_arg = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-tuple", {true})
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "tuple length mismatch")
end

-- record: invalid type
g.test_record_invalid_arg = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-record", "not a table")
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected map for record")
end

g.test_record_missing_field = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-record", {x = 42})
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(
        tostring(err),
        "missing field `y` in record"
    )
end

-- variant: not a map
g.test_variant_invalid_type = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-variant", "not a variant")
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected map for variant")
end

-- variant: multiple keys
g.test_variant_multiple_keys = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-variant",
            {none = true, any = true})
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(
        tostring(err),
        "expected exactly one key in variant"
    )
end

-- variant: unknown tag
g.test_variant_unknown_tag = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-variant", {foobar = true})
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(
        tostring(err),
        "unknown variant case: foobar"
    )
end

-- enum: invalid value
g.test_enum_invalid_arg = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-enum", "not-a-color")
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(
        tostring(err),
        "invalid enum case: not-a-color"
    )
end

-- flags: not an array
g.test_flags_invalid_type = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-flags", "get")
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected array for flags")
end

-- flags: unknown value
g.test_flags_invalid_value = function(cg)
    local ok, err = pcall(function()
        return wasm.call(cg.m, "t-flags", {"get", "noop"})
    end)
    t.assert_equals(ok, false)
    t.assert_str_contains(
        tostring(err),
        "invalid flag name: noop"
    )
end
