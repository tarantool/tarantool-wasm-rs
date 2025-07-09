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

g.test_s64 = function()
    local res = m:t_s64(1)
    t.assert_equals(res, 1)
end

g.test_u64 = function()
    local res = m:t_u64(1)
    t.assert_equals(res, 1)
end

g.test_bool = function()
    local res = m:t_bool(true)
    t.assert_equals(res, true)
    res = m:t_bool(false)
    t.assert_equals(res, false)
end

g.test_string = function()
    local res = m:t_string("string")
    t.assert_equals(res, "string")
end
