local t = require('luatest')
local luawasm = require('luawasm')

local g = t.group('basic_export')
local m

local function get_test_dir()
    local info = debug.getinfo(1, 'S')
    local file_path = info.source:sub(2)
    local base_dir = file_path:match("(.*/)")
    return base_dir
end

g.before_all(function()
    local path = get_test_dir() .. "wasm_component_test/basic_export_test"
    m = luawasm:new({ dir = path })
end)

-- boolean
g.test_bool_true = function()
    local res = m:t_bool(true)
    t.assert_equals(res, true)
end

g.test_bool_false = function()
    local res = m:t_bool(false)
    t.assert_equals(res, false)
end

-- signed integers
g.test_s8 = function()
    local res = m:t_s8(-100)
    t.assert_equals(res, -100)
end

g.test_s16 = function()
    local res = m:t_s16(-12345)
    t.assert_equals(res, -12345)
end

g.test_s32 = function()
    local res = m:t_s32(-123456789)
    t.assert_equals(res, -123456789)
end

g.test_s64 = function()
    local res = m:t_s64(-123456789012345)
    t.assert_equals(res, -123456789012345)
end

-- unsigned integers
g.test_u8 = function()
    local res = m:t_u8(255)
    t.assert_equals(res, 255)
end

g.test_u16 = function()
    local res = m:t_u16(65535)
    t.assert_equals(res, 65535)
end

g.test_u32 = function()
    local res = m:t_u32(4294967295)
    t.assert_equals(res, 4294967295)
end

g.test_u64 = function()
    local res = m:t_u64(123456789012345)
    t.assert_equals(res, 123456789012345)
end

-- floats
g.test_f32 = function()
    local res = m:t_f32(3.14)
    t.assert_almost_equals(res, 3.14, 1e-6)
end

g.test_f64 = function()
    local res = m:t_f64(2.718281828459)
    t.assert_almost_equals(res, 2.718281828459, 1e-12)
end

-- char
g.test_char = function()
    local res = m:t_char("a")
    t.assert_equals(res, "a")
end

-- string
g.test_string = function()
    local res = m:t_string("test_string")
    t.assert_equals(res, "test_string")
end

-- list<string>
g.test_list_empty = function()
    local res = m:t_list({})
    t.assert_equals(res, {})
end

g.test_list_not_empty = function()
    local res = m:t_list({"s1", "s2", "s3"})
    t.assert_equals(res, {"s1", "s2", "s3"})
end

-- option<u8>
g.test_option_none = function()
    local res = m:t_option(nil)
    t.assert_equals(res, nil)
end

g.test_option_not_none = function()
    local res = m:t_option(255)
    t.assert_equals(res, 255)
end

-- result<bool, string>
g.test_result_ok = function()
    local res = m:t_result(true)
    t.assert_equals(true, res[1])
    t.assert_equals(true, res[2])
end

g.test_result_error = function()
    -- Before this test, it's important to understand that the behavior of `result<T, E>` in WebAssembly
    -- components can vary significantly depending on the source language used to implement the component.
    -- Some languages raise exceptions natively (like Python), while others return errors as structured
    -- values (like C). This affects how the Tarantool host receives and handles these results.
    --
    -- Always use `pcall(m:t_result(...))` or wrap calls in a helper if you're unsure whether the component
    -- may raise exceptions. Use direct pattern matching only when you're confident the backend uses safe
    -- return-based error handling.
    local ok, res = pcall(function()
        return m:t_result(false)
    end)
    if not ok then
        -- pcall itself failed (e.g. backend panicked and raised a trap)
        t.assert_equals(ok, false)
        t.assert_equals(type(res), "userdata")
    else
        t.assert_equals(ok, false)
        t.assert_equals(res[1], false)
        t.assert_equals(res[2], "false")
    end
end

-- tuple
g.test_tuple = function()
    local res = m:t_tuple({false, 3.14, "tuple"})
    t.assert_equals(res, {false, "tuple"})
end

-- variant
g.test_variant_none = function()
    local res = m:t_variant({none = true})
    t.assert_equals(res, {})
end

g.test_variant_bool = function()
    local res = m:t_variant({any = true})
    t.assert_equals(res, {any = true})
end

g.test_variant_list = function()
    local res = m:t_variant({ restricted = {"s1", "s2"} })
    t.assert_equals(res, {restricted = {"s1", "s2"}})
end

-- enum
g.test_enum = function()
    local res

    res = m:t_enum("red")
    t.assert_equals(res, "red")

    res = m:t_enum("black")
    t.assert_equals(res, "black")

    res = m:t_enum("navy-blue")
    t.assert_equals(res, "navy-blue")
end

-- flags
g.test_flags = function()
    local res

    res = m:t_flags({})
    t.assert_equals(res, {})

    res = m:t_flags({ "get", "put"})
    t.assert_equals(res, { "get", "put" })
end
