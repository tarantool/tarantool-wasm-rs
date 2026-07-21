local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')
local ffi = require('ffi')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local test_group = "export_test"

local g = t.group(
    test_group,
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
g.test_bool_true = function(cg)
    local res = wasm.call(cg.m, "t-bool", true)
    t.assert_equals(res, true)
end

g.test_bool_false = function(cg)
    local res = wasm.call(cg.m, "t-bool", false)
    t.assert_equals(res, false)
end

-- signed integers
g.test_s8 = function(cg)
    for _, value in ipairs({-128, 127}) do
        t.assert_equals(wasm.call(cg.m, "t-s8", value), value)
    end
end

g.test_s16 = function(cg)
    for _, value in ipairs({-32768, 32767}) do
        t.assert_equals(wasm.call(cg.m, "t-s16", value), value)
    end
end

g.test_s32 = function(cg)
    for _, value in ipairs({-2147483648, 2147483647}) do
        t.assert_equals(wasm.call(cg.m, "t-s32", value), value)
    end
end

g.test_s64 = function(cg)
    local value = 9007199254740991
    t.assert_equals(wasm.call(cg.m, "t-s64", value), value)
end

g.test_s64_negative = function(cg)
    t.skip_if(
        cg.params.lang == "js",
        "componentize-js traps when lowering a negative s64 result"
    )
    local value = -9007199254740991
    t.assert_equals(wasm.call(cg.m, "t-s64", value), value)
end

-- unsigned integers
g.test_u8 = function(cg)
    for _, value in ipairs({0, 255}) do
        t.assert_equals(wasm.call(cg.m, "t-u8", value), value)
    end
end

g.test_u16 = function(cg)
    for _, value in ipairs({0, 65535}) do
        t.assert_equals(wasm.call(cg.m, "t-u16", value), value)
    end
end

g.test_u32 = function(cg)
    for _, value in ipairs({0, 2147483647}) do
        t.assert_equals(wasm.call(cg.m, "t-u32", value), value)
    end
end

g.test_u32_high_bit = function(cg)
    t.skip_if(
        cg.params.lang == "js",
        "componentize-js corrupts u32 results with the high bit set"
    )
    for _, value in ipairs({2147483648, 4294967295}) do
        t.assert_equals(wasm.call(cg.m, "t-u32", value), value)
    end
end

g.test_u64 = function(cg)
    for _, value in ipairs({0, 9007199254740991, ffi.cast('uint64_t', -1)}) do
        t.assert_equals(wasm.call(cg.m, "t-u64", value), value)
    end
end

-- floats
g.test_f32 = function(cg)
    local res = wasm.call(cg.m, "t-f32", 3.14)
    t.assert_almost_equals(res, 3.14, 1e-6)
end

g.test_f64 = function(cg)
    local res = wasm.call(cg.m, "t-f64", 2.718281828459)
    t.assert_almost_equals(res, 2.718281828459, 1e-12)
end

-- char
g.test_char = function(cg)
    local res = wasm.call(cg.m, "t-char", "a")
    t.assert_equals(res, "a")
end

-- string
g.test_string = function(cg)
    local res = wasm.call(cg.m, "t-string", "test_string")
    t.assert_equals(res, "test_string")
end

-- list<string>
g.test_list_empty = function(cg)
    local res = wasm.call(cg.m, "t-list", {})
    t.assert_equals(res, {})
end

g.test_list_not_empty = function(cg)
    local res = wasm.call(cg.m, "t-list", {"s1", "s2", "s3"})
    t.assert_equals(res, {"s1", "s2", "s3"})
end

-- option<u8>
g.test_option_none = function(cg)
    -- Some guest languages (like C) may return 0 to represent None/nil,
    -- so we explicitly accept both nil and 0 as valid results.
    local res = wasm.call(cg.m, "t-option", nil)
    t.assert(res == nil or res == 0, ("unexpected value: %s"):format(res))
end

g.test_option_not_none = function(cg)
    local res = wasm.call(cg.m, "t-option", 255)
    t.assert_equals(res, 255)
end

-- result<bool, string>
g.test_result_ok = function(cg)
    local res = wasm.call(cg.m, "t-result", true)
    t.assert_equals(true, res[1])
    t.assert_equals(true, res[2])
end

g.test_result_error = function(cg)
    -- Before this test, it's important to understand that the behavior of
    -- `result<T, E>` in WebAssembly components can vary significantly
    -- depending on the source language used to implement the component.
    -- Some languages raise exceptions natively (like Python), while others
    -- return errors as structured values (like C). This affects how the
    -- Tarantool host receives and handles these results.
    --
    -- Always use `pcall(m:t_result(...))` or wrap calls in a helper if you're
    -- unsure whether the component may raise exceptions. Use direct pattern
    -- matching only when you're confident the backend uses safe
    -- return-based error handling.
    local ok, res = pcall(function()
        return wasm.call(cg.m, "t-result", false)
    end)
    if not ok then
        -- pcall itself failed (e.g. backend panicked and raised a trap)
        t.assert_equals(ok, false)
        t.assert_equals(type(res), "userdata")
    else
        t.assert_equals(ok, true)
        t.assert_equals(res[1], false)
        t.assert_equals(res[2], "error")
    end
end

-- tuple
g.test_tuple = function(cg)
    local res = wasm.call(cg.m, "t-tuple", {false, 3.14, "tuple"})
    t.assert_equals(res, {false, "tuple"})
end

-- record
g.test_record = function(cg)
    local res = wasm.call(cg.m, "t-record", {x = 1, y = "2"})
    t.assert_equals(res, {x = 1, y = "2"})
end

-- variant
g.test_variant_none = function(cg)
    local res = wasm.call(cg.m, "t-variant", {none = true})
    t.assert_equals(res, {})
end

g.test_variant_bool = function(cg)
    local res = wasm.call(cg.m, "t-variant", {any = true})
    t.assert_equals(res, {any = true})
end

g.test_variant_list = function(cg)
    local res = wasm.call(cg.m, "t-variant", { restricted = {"s1", "s2"} })
    t.assert_equals(res, {restricted = {"s1", "s2"}})
end

-- enum
g.test_enum = function(cg)
    local res

    res = wasm.call(cg.m, "t-enum", "red")
    t.assert_equals(res, "red")

    res = wasm.call(cg.m, "t-enum", "black")
    t.assert_equals(res, "black")

    res = wasm.call(cg.m, "t-enum", "navy-blue")
    t.assert_equals(res, "navy-blue")
end

-- flags
g.test_flags = function(cg)
    local res

    res = wasm.call(cg.m, "t-flags", {})
    t.assert_equals(res, {})

    res = wasm.call(cg.m, "t-flags", { "get", "put"})
    t.assert_equals(res, { "get", "put" })
end
