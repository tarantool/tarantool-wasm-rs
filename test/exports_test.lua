local t = require('luatest')
local luawasm = require('luawasm')
local fio = require('fio')

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
g.test_bool_true = function(cg)
    local res = cg.m:t_bool(true)
    t.assert_equals(res, true)
end

g.test_bool_false = function(cg)
    local res = cg.m:t_bool(false)
    t.assert_equals(res, false)
end

-- signed integers
g.test_s8 = function(cg)
    local res = cg.m:t_s8(-100)
    t.assert_equals(res, -100)
end

g.test_s16 = function(cg)
    local res = cg.m:t_s16(-12345)
    t.assert_equals(res, -12345)
end

g.test_s32 = function(cg)
    local res = cg.m:t_s32(-123456789)
    t.assert_equals(res, -123456789)
end

g.test_s64 = function(cg)
    t.skip_if(
        cg.params.lang == "js",
        "JS Number cannot safely represent full range of 64-bit integers; use BigInt or another language for exact s64 support"
    )

    local res = cg.m:t_s64(-123456789012345)
    t.assert_equals(res, -123456789012345)
end

-- unsigned integers
g.test_u8 = function(cg)
    local res = cg.m:t_u8(255)
    t.assert_equals(res, 255)
end

g.test_u16 = function(cg)
    local res = cg.m:t_u16(65535)
    t.assert_equals(res, 65535)
end

g.test_u32 = function(cg)
    t.skip_if(
        cg.params.lang == "js",
        "Skipped due to JS Number precision limits; values above 2^53 may lose precision when roundtripped through wasm bindings"
    )

    local res = cg.m:t_u32(4294967295)
    t.assert_equals(res, 4294967295)
end

g.test_u64 = function(cg)
    local res = cg.m:t_u64(123456789012345)
    t.assert_equals(res, 123456789012345)
end

-- floats
g.test_f32 = function(cg)
    local res = cg.m:t_f32(3.14)
    t.assert_almost_equals(res, 3.14, 1e-6)
end

g.test_f64 = function(cg)
    local res = cg.m:t_f64(2.718281828459)
    t.assert_almost_equals(res, 2.718281828459, 1e-12)
end

-- char
g.test_char = function(cg)
    local res = cg.m:t_char("a")
    t.assert_equals(res, "a")
end

-- string
g.test_string = function(cg)
    local res = cg.m:t_string("test_string")
    t.assert_equals(res, "test_string")
end

-- list<string>
g.test_list_empty = function(cg)
    local res = cg.m:t_list({})
    t.assert_equals(res, {})
end

g.test_list_not_empty = function(cg)
    local res = cg.m:t_list({"s1", "s2", "s3"})
    t.assert_equals(res, {"s1", "s2", "s3"})
end

-- option<u8>
g.test_option_none = function(cg)
    -- Some guest languages (like C) may return 0 to represent None/nil,
    -- so we explicitly accept both nil and 0 as valid results.
    local res = cg.m:t_option(nil)
    t.assert(res == nil or res == 0, ("unexpected value: %s"):format(res))
end

g.test_option_not_none = function(cg)
    local res = cg.m:t_option(255)
    t.assert_equals(res, 255)
end

-- result<bool, string>
g.test_result_ok = function(cg)
    local res = cg.m:t_result(true)
    t.assert_equals(true, res[1])
    t.assert_equals(true, res[2])
end

g.test_result_error = function(cg)
    -- Before this test, it's important to understand that the behavior of `result<T, E>` in WebAssembly
    -- components can vary significantly depending on the source language used to implement the component.
    -- Some languages raise exceptions natively (like Python), while others return errors as structured
    -- values (like C). This affects how the Tarantool host receives and handles these results.
    --
    -- Always use `pcall(m:t_result(...))` or wrap calls in a helper if you're unsure whether the component
    -- may raise exceptions. Use direct pattern matching only when you're confident the backend uses safe
    -- return-based error handling.
    local ok, res = pcall(function() return cg.m:t_result(false) end)
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
    local res = cg.m:t_tuple({false, 3.14, "tuple"})
    t.assert_equals(res, {false, "tuple"})
end

-- record
g.test_record = function(cg)
    local res = cg.m:t_record({x = 1, y = "2"})
    t.assert_equals(res, {x = 1, y = "2"})
end

-- variant
g.test_variant_none = function(cg)
    local res = cg.m:t_variant({none = true})
    t.assert_equals(res, {})
end

g.test_variant_bool = function(cg)
    local res = cg.m:t_variant({any = true})
    t.assert_equals(res, {any = true})
end

g.test_variant_list = function(cg)
    local res = cg.m:t_variant({ restricted = {"s1", "s2"} })
    t.assert_equals(res, {restricted = {"s1", "s2"}})
end

-- enum
g.test_enum = function(cg)
    local res

    res = cg.m:t_enum("red")
    t.assert_equals(res, "red")

    res = cg.m:t_enum("black")
    t.assert_equals(res, "black")

    res = cg.m:t_enum("navy-blue")
    t.assert_equals(res, "navy-blue")
end

-- flags
g.test_flags = function(cg)
    local res

    res = cg.m:t_flags({})
    t.assert_equals(res, {})

    res = cg.m:t_flags({ "get", "put"})
    t.assert_equals(res, { "get", "put" })
end
