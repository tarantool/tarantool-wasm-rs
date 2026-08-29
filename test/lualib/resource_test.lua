local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')
local fiber = require('fiber')

package.path = './test/?.lua;' .. package.path
local utils = require('utils')

local g = t.group('wit_resources')

local function resource_component_path()
    return fio.pathjoin(
        utils.get_test_dir(), 'lualib', 'wasm_component_test',
        'resource_test', 'resource.wasm'
    )
end

g.before_all(function(cg)
    cg.module = wasm.load(resource_component_path())
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

g.test_resources_cannot_cross_instances = function(cg)
    local other = wasm.bind(cg.module)
    local counter = cg.counters.counter.new(9)

    local ok, err = pcall(
        other['test:resources/counters@0.1.0'].peek,
        counter
    )
    t.assert_not(ok)
    t.assert_str_contains(tostring(err), 'different WASM instance')
    t.assert(counter:drop())
end

g.test_interpreter_bindings_share_resources = function()
    local first_module = wasm.load(
        resource_component_path(), {interpreter = true}
    )
    local second_module = wasm.load(
        resource_component_path(), {interpreter = true}
    )
    local first = wasm.bind(first_module)
    local second = wasm.bind(second_module)
    local first_counters = first['test:resources/counters@0.1.0']
    local second_counters = second['test:resources/counters@0.1.0']

    local counter = first_counters.counter.new(21)
    t.assert_equals(second_counters.peek(counter), 21)

    t.assert(wasm.interpreter_exec(first_module, [[
def read_latest():
    return latest_value()
]]))
    t.assert_equals(second.interpreter_call('read_latest'), 21)

    t.assert_equals(counter:add(1), 22)
    t.assert_equals(
        wasm.interpreter_call(second_module, 'read_latest'), 22
    )

    t.assert(second.interpreter_exec([[
def double_latest():
    return add_to_latest(latest_value())
]]))
    t.assert_equals(
        wasm.interpreter_call(first_module, 'double_latest'), 44
    )
    t.assert_equals(counter:get(), 44)

    t.assert(wasm.drop(first_module))
    t.assert_equals(second_counters.peek(counter), 44)
    t.assert(counter:drop())
    t.assert(wasm.drop(second_module))
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

    local consumed = wasm.session_call(
        session, selector .. '[constructor]counter', 17
    )
    t.assert_equals(wasm.session_call(session, selector .. 'consume', consumed), 17)
    t.assert_not(consumed:is_alive())

    t.assert(first:drop())
    t.assert(second:drop())
    t.assert(wasm.close_session(session))
end

g.test_cancelled_session_call_still_records_consumed_resource = function(cg)
    local session = wasm.open_session(cg.module)
    local selector = 'test:resources/counters@0.1.0::'
    local counter = wasm.session_call(
        session, selector .. '[constructor]counter', 42
    )
    local calls = {}
    for _ = 1, 2000 do
        table.insert(calls, {selector .. 'peek', counter})
    end

    local blocker_done = fiber.channel(1)
    fiber.create(function()
        blocker_done:put({pcall(wasm.session_batch, session, calls)})
    end)

    local consumer_started = fiber.channel(1)
    local consumer_done = fiber.channel(1)
    local consumer = fiber.create(function()
        consumer_started:put(true)
        consumer_done:put({
            pcall(wasm.session_call, session, selector .. 'consume', counter),
        })
    end)
    t.assert(consumer_started:get(1), 'session consumer did not start')
    consumer:cancel()

    local blocker_result = blocker_done:get(30)
    t.assert(blocker_result ~= nil, 'blocking session batch timed out')
    t.assert(blocker_result[1], tostring(blocker_result[2]))
    local consumer_result = consumer_done:get(30)
    t.assert(consumer_result ~= nil, 'cancelled session call timed out')
    t.assert_not(consumer_result[1])
    t.assert_str_contains(tostring(consumer_result[2]), 'fiber is cancelled')
    local cleanup_deadline = fiber.clock() + 5
    while counter:is_alive() and fiber.clock() < cleanup_deadline do
        fiber.sleep(0.001)
    end
    t.assert_not(counter:is_alive())
    t.assert(wasm.close_session(session))
end
