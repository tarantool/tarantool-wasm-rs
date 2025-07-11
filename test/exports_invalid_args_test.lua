local t = require('luatest')
local luawasm = require('luawasm')
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
    local path = fio.pathjoin(utils.get_test_dir(), "wasm_component_test", test_group, cg.params.lang)
    local wasm
    if cg.params.lang == "c" then
        wasm = "adder.component.wasm"
    end
    cg.m = luawasm:new({
        dir = path,
        wasm = wasm,
    })
end)

-- boolean
g.test_bool_invalid_arg = function(cg)
    -- Any non-boolean input (including strings, numbers, etc.) is interpreted as `false`.
    -- Only `true` is preserved as `true`; all other values convert to `false`.
    local ok, res = pcall(function()
        return cg.m:t_bool("bool")
    end)
    t.assert_equals(ok, true)
    t.assert_equals(res, false)
end

-- signed integers
g.test_s8_invalid_arg = function(cg)
    local ok, err = pcall(function() return cg.m:t_s8("x") end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected s8")
end

g.test_s16_invalid_arg = function(cg)
    local ok, err = pcall(function() return cg.m:t_s16("x") end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected s16")
end

g.test_s32_invalid_arg = function(cg)
    local ok, err = pcall(function() return cg.m:t_s32("x") end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected s32")
end

g.test_s64_invalid_arg = function(cg)
    local ok, err = pcall(function() return cg.m:t_s64("x") end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected s64")
end


-- unsigned integers
g.test_u8_invalid_arg = function(cg)
    local ok, err = pcall(function() return cg.m:t_u8("not a number") end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected u8")
end

g.test_u16_invalid_arg = function(cg)
    local ok, err = pcall(function() return cg.m:t_u16("not a number") end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected u16")
end

g.test_u32_invalid_arg = function(cg)
    local ok, err = pcall(function() return cg.m:t_u32("not a number") end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected u32")
end

g.test_u64_invalid_arg = function(cg)
    local ok, err = pcall(function() return cg.m:t_u64("not a number") end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected u64")
end

-- floats
g.test_f32_invalid_arg = function(cg)
    local ok, err = pcall(function() return cg.m:t_f32({}) end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected f32")
end

g.test_f64_invalid_arg = function(cg)
    local ok, err = pcall(function() return cg.m:t_f64({}) end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected f64")
end

-- char
g.test_char_invalid_arg = function(cg)
    local ok, err = pcall(function() return cg.m:t_char("abc") end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected single character string")
end

-- string
g.test_string_invalid_arg = function(cg)
    local ok, err = pcall(function() return cg.m:t_string(123) end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected string")
end

-- list<string>
g.test_list_invalid_arg = function(cg)
    local ok, err = pcall(function() return cg.m:t_list("not a list") end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected array for list")
end

-- tuple 
g.test_tuple_invalid_arg = function(cg)
    local ok, err = pcall(function() return cg.m:t_tuple({true}) end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "tuple length mismatch")
end

-- record: invalid type
g.test_record_invalid_arg = function(cg)
    local ok, err = pcall(function() return cg.m:t_record("not a table") end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected map for record")
end

g.test_record_missing_field = function(cg)
    local ok, err = pcall(function() return cg.m:t_record({x = 42}) end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "missing field `y` in record")
end

-- variant: not a map
g.test_variant_invalid_type = function(cg)
    local ok, err = pcall(function() return cg.m:t_variant("not a variant") end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected map for variant")
end

-- variant: multiple keys
g.test_variant_multiple_keys = function(cg)
    local ok, err = pcall(function() return cg.m:t_variant({none = true, any = true}) end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected exactly one key in variant")
end

-- variant: unknown tag
g.test_variant_unknown_tag = function(cg)
    local ok, err = pcall(function() return cg.m:t_variant({foobar = true}) end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "unknown variant case: foobar")
end

-- enucg.m: invalid value
g.test_enum_invalid_arg = function(cg)
    local ok, err = pcall(function() return cg.m:t_enum("not-a-color") end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "invalid enum case: not-a-color")
end

-- flags: not an array
g.test_flags_invalid_type = function(cg)
    local ok, err = pcall(function() return cg.m:t_flags("get") end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "expected array for flags")
end

-- flags: unknown value
g.test_flags_invalid_value = function(cg)
    local ok, err = pcall(function() return cg.m:t_flags({ "get", "noop" }) end)
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), "invalid flag name: noop")
end
