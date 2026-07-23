local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')

package.path = './test/?.lua;' .. package.path
local utils = require('utils')

local g = t.group('runtime_stats')
local source = fio.pathjoin(
    utils.get_test_dir(),
    'lualib',
    'wasm_component_test',
    'export_test',
    'c',
    'adder.component.wasm'
)

local function copy_file(source_path, target_path)
    local input = assert(io.open(source_path, 'rb'))
    local contents = input:read('*a')
    input:close()
    local output = assert(io.open(target_path, 'wb'))
    output:write(contents)
    output:close()
end

g.before_each(function(cg)
    cg.tmpdir = fio.tempdir()
    cg.path = fio.pathjoin(cg.tmpdir, 'stats-adder.wasm')
    copy_file(source, cg.path)
    cg.modules = {}
    cg.sessions = {}
end)

g.after_each(function(cg)
    for _, session in ipairs(cg.sessions) do
        pcall(wasm.close_session, session)
    end
    for _, module in ipairs(cg.modules) do
        pcall(wasm.drop, module)
    end
    fio.rmtree(cg.tmpdir)
end)

g.test_snapshot_shape_and_execution_counters = function(cg)
    local before = wasm.stats()

    t.assert_type(before.compilation, 'table')
    t.assert_type(before.instantiation, 'table')
    t.assert_type(before.guest_execution, 'table')
    t.assert_type(before.tx_wait, 'table')
    t.assert_type(before.lua_callback, 'table')
    t.assert_type(before.fuel, 'table')
    t.assert_type(before.current, 'table')
    t.assert_type(before.cache.components, 'table')
    t.assert_type(before.cache.exports, 'table')
    t.assert_type(before.traps.by_kind, 'table')
    t.assert_type(before.conversion_errors.by_kind, 'table')
    t.assert_type(before.memory, 'table')

    local first = wasm.load(cg.path, {max_instructions = 1000000})
    local second = wasm.load(cg.path, {max_instructions = 1000000})
    table.insert(cg.modules, first)
    table.insert(cg.modules, second)

    t.assert_equals(
        wasm.call(first, 't-s16', 7),
        7
    )
    t.assert_equals(
        wasm.call(first, 't-s16', 8),
        8
    )

    local after = wasm.stats()
    t.assert_ge(after.compilation.count, before.compilation.count + 1)
    t.assert_ge(after.instantiation.count, before.instantiation.count + 2)
    t.assert_ge(after.guest_execution.count, before.guest_execution.count + 2)
    t.assert_ge(after.cache.components.misses, before.cache.components.misses + 1)
    t.assert_ge(after.cache.components.hits, before.cache.components.hits + 1)
    t.assert_ge(after.cache.exports.misses, before.cache.exports.misses + 1)
    t.assert_ge(after.cache.exports.hits, before.cache.exports.hits + 1)
    t.assert_ge(after.fuel.executions, before.fuel.executions + 2)
    t.assert_gt(after.fuel.consumed, before.fuel.consumed)
    t.assert_ge(after.memory.peak_bytes, after.memory.current_bytes)
    t.assert_ge(after.current.modules, before.current.modules + 2)
end

g.test_sessions_and_conversion_errors = function(cg)
    local module = wasm.load(cg.path)
    table.insert(cg.modules, module)

    local before = wasm.stats()
    local session = wasm.open_session(module)
    table.insert(cg.sessions, session)
    local opened = wasm.stats()
    t.assert_equals(opened.current.sessions, before.current.sessions + 1)

    local ok = pcall(
        wasm.session_call,
        session,
        't-s16',
        'not-an-integer'
    )
    t.assert_not(ok)
    local failed = wasm.stats()
    t.assert_ge(
        failed.conversion_errors.total,
        opened.conversion_errors.total + 1
    )
    t.assert_ge(
        failed.conversion_errors.by_kind.resource_arguments,
        1
    )

    t.assert(wasm.close_session(session))
    cg.sessions = {}
    t.assert_equals(wasm.stats().current.sessions, before.current.sessions)
end
