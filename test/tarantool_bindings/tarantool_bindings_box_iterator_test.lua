local t = require("luatest")
local wasm = require("wasm")
local fio = require("fio")

package.path = "./test/?.lua;" .. package.path
local utils = require("utils")

local g = t.group("box_iterator_test")

local function read_file(path)
    local f = assert(io.open(path, "r"))
    local data = f:read("*a")
    f:close()
    return data
end

g.after_all(function()
    utils.cleanup_logs(fio.cwd())
end)

local wasm_path = fio.pathjoin(utils.get_test_dir(), "tarantool_bindings", 
                         "wasm_component_test", "box_iterator_test", "adder.wasm")

g.test_box_iterator = function()
    box.cfg{}

    box.schema.space.create("tester", { if_not_exists = true })
    box.space.tester:format({ { name = "id", type = "unsigned" }, { name = "value", type = "string" } })
    box.space.tester:create_index("primary", { if_not_exists = true })
    box.space.tester:truncate()

    box.space.tester:insert({ 1, "a" })
    box.space.tester:insert({ 2, "b" })
    box.space.tester:insert({ 3, "c" })

    local out = fio.pathjoin(fio.tempdir(), "out.txt")

    local m = wasm.load(wasm_path, {
        stdout = out
    })

    local handler = wasm.run(m)
    t.assert(wasm.join(handler))

    local expected = '[1,"a"]\n[2,"b"]\n[3,"c"]\n'
    local content = read_file(out)
    t.assert_equals(content,  expected)
end
