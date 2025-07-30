local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local g = t.group("component_exports")

g.test_get_component_exports = function()
    local path = fio.pathjoin(utils.get_test_dir(), "lualib", "wasm_component_test", "component_exports_test", "adder.wasm")
    local m = wasm.load(path)
    local exports = wasm.exports(m)
    
    local expected = {
        ["docs:adder/nums@0.1.0"] = {
            ["t-bool"] = {params = {{"x", "bool"}}, result = {"bool"}},
            ["t-f32"] = {params = {{"x", "f32"}}, result = {"f32"}},
            ["t-f64"] = {params = {{"x", "f64"}}, result = {"f64"}},
            ["t-s16"] = {params = {{"x", "s16"}}, result = {"s16"}},
            ["t-s32"] = {params = {{"x", "s32"}}, result = {"s32"}},
            ["t-s64"] = {params = {{"x", "s64"}}, result = {"s64"}},
            ["t-s8"] = {params = {{"x", "s8"}}, result = {"s8"}},
            ["t-u16"] = {params = {{"x", "u16"}}, result = {"u16"}},
            ["t-u32"] = {params = {{"x", "u32"}}, result = {"u32"}},
            ["t-u64"] = {params = {{"x", "u64"}}, result = {"u64"}},
            ["t-u8"] = {params = {{"x", "u8"}}, result = {"u8"}},
        },
        ["t-char"] = {params = {{"x", "char"}}, result = {"char"}},
        ["t-enum"] = {
            params = {{"x", "enum(red | black | navy-blue)"}},
            result = {"enum(red | black | navy-blue)"},
        },
        ["t-flags"] = {
            params = {{"x", "flags(get, post, put, delete)"}},
            result = {"flags(get, post, put, delete)"},
        },
        ["t-list"] = {params = {{"x", "list<string>"}}, result = {"list<string>"}},
        ["t-option"] = {params = {{"x", "option<u8>"}}, result = {"option<u8>"}},
        ["t-record"] = {
            params = {{"x", "record{x: u64, y: string}"}},
            result = {"record{x: u64, y: string}"},
        },
        ["t-result"] = {params = {{"x", "bool"}}, result = {"result<bool, string>"}},
        ["t-string"] = {params = {{"x", "string"}}, result = {"string"}},
        ["t-tuple"] = {
            params = {{"x", "tuple<bool, f32, string>"}},
            result = {"tuple<bool, string>"},
        },
        ["t-variant"] = {
            params = {{"x", "variant(none | any(bool) | restricted(list<string>))"}},
            result = {"variant(none | any(bool) | restricted(list<string>))"},
        },
    }

    t.assert_equals(exports, expected)
end