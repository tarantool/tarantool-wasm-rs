local fio = require("fio")
local t = require("luatest")
local wasm = require("wasm")

package.path = "./test/?.lua;" .. package.path
local utils = require("utils")

local g = t.group("read_view_ee_test")

local function read_file(path)
    local file = assert(io.open(path, "r"))
    local data = file:read("*a")
    file:close()
    return data
end

g.after_all(function()
    utils.cleanup_logs(fio.cwd())
end)

g.test_snapshot_get_count_and_iterator = function()
    t.skip_if(
        require('tarantool').package ~= 'Tarantool Enterprise',
        'requires Tarantool Enterprise Edition'
    )
    box.cfg({})
    if box.space.read_view_wasm_test ~= nil then
        box.space.read_view_wasm_test:drop()
    end
    local space = box.schema.space.create("read_view_wasm_test", { id = 777 })
    space:format({
        { name = "id", type = "unsigned" },
        { name = "value", type = "string" },
    })
    space:create_index("primary")
    space:insert({ 1, "a" })
    space:insert({ 2, "b" })

    local component = fio.pathjoin(
        utils.get_test_dir(),
        "tarantool_bindings",
        "wasm_component_test",
        "bindings_read_view_ee_test",
        "adder.wasm"
    )
    local output = fio.pathjoin(fio.tempdir(), "read-view-output.txt")
    local module = wasm.load(component, { stdout = output })
    local handle = wasm.run(module)
    t.assert(wasm.join(handle))

    t.assert_equals(read_file(output), table.concat({
        'enterprise',
        '[1, "a"]',
        '[1, "a"]',
        '2',
        '[[1,"a"],[2,"b"]]',
        '',
    }, '\n'))
    t.assert_equals(space:get({ 1 }), { 1, "new" })
    t.assert_equals(space:get({ 3 }), { 3, "c" })

    space:drop()
end

g.test_community_edition_reports_unsupported = function()
    t.skip_if(
        require('tarantool').package == 'Tarantool Enterprise',
        'requires Tarantool Community Edition'
    )
    box.cfg({})
    local component = fio.pathjoin(
        utils.get_test_dir(),
        "tarantool_bindings",
        "wasm_component_test",
        "bindings_read_view_ee_test",
        "adder.wasm"
    )
    local output = fio.pathjoin(fio.tempdir(), "read-view-ce-output.txt")
    local module = wasm.load(component, { stdout = output })
    local handle = wasm.run(module)
    t.assert(wasm.join(handle))
    t.assert_equals(read_file(output), table.concat({
        'community',
        'raw read-view API requires Tarantool Enterprise Edition',
        '',
    }, '\n'))
end
