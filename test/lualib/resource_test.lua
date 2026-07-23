local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')

package.path = './test/?.lua;' .. package.path
local utils = require('utils')

local g = t.group('wit_resources')

g.before_all(function(cg)
    local path = fio.pathjoin(
        utils.get_test_dir(), 'lualib', 'wasm_component_test',
        'resource_test', 'resource.wasm'
    )
    cg.module = wasm.load(path)
    cg.bound = wasm.bind(cg.module)
    cg.counters = cg.bound['test:resources/counters@0.1.0']
end)

g.after_all(function(cg)
    wasm.drop(cg.module)
end)

g.test_constructor_and_methods = function(cg)
    local counter = cg.counters.counter.new(10)

    t.assert_equals(counter:type(), 'counter')
    t.assert(counter:owned())
    t.assert(counter:is_alive())
    t.assert_equals(counter:get(), 10)
    t.assert_equals(counter:add(7), 17)
    t.assert_equals(cg.counters.peek(counter), 17)

    t.assert(counter:drop())
    t.assert_not(counter:is_alive())
    t.assert_not(counter:drop())
    local ok, err = pcall(counter.get, counter)
    t.assert_not(ok)
    t.assert_str_contains(tostring(err), 'WIT resource has been dropped')
end

g.test_owned_resource_is_consumed = function(cg)
    local counter = cg.counters.counter.new(42)
    t.assert_equals(cg.counters.consume(counter), 42)
    t.assert_not(counter:is_alive())

    local ok, err = pcall(cg.counters.peek, counter)
    t.assert_not(ok)
    t.assert_str_contains(tostring(err), 'WIT resource has been dropped')
end

g.test_nested_resource_results = function(cg)
    local pair = cg.counters.pair(3, 5)
    t.assert_equals(pair[1]:get(), 3)
    t.assert_equals(pair[2]:get(), 5)
    t.assert_equals(cg.counters.counter.sum(pair[1], pair[2]), 8)
    t.assert(pair[1]:drop())
    t.assert(pair[2]:drop())
end

g.test_resources_cannot_cross_sessions = function(cg)
    local other = wasm.bind(cg.module)
    local counter = cg.counters.counter.new(9)

    local ok, err = pcall(
        other['test:resources/counters@0.1.0'].peek,
        counter
    )
    t.assert_not(ok)
    t.assert_str_contains(tostring(err), 'different WASM session')
    t.assert(counter:drop())
end

g.test_explicit_session_api = function(cg)
    local session = wasm.open_session(cg.module)
    local selector = 'test:resources/counters@0.1.0::'
    local first = wasm.session_call(
        session, selector .. '[constructor]counter', 11
    )
    local second = wasm.session_call(
        session, selector .. '[constructor]counter', 13
    )

    local values = wasm.session_batch(session, {
        {selector .. 'peek', first},
        {selector .. '[static]counter.sum', first, second},
    })
    t.assert_equals(values, {11, 24})

    t.assert(first:drop())
    t.assert(second:drop())
    t.assert(wasm.close_session(session))
end
