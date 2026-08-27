local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local g = t.group('interpreter_api')

local wasm_path = fio.pathjoin(
    utils.get_test_dir(),
    '..',
    'python-interpreter',
    'adder.wasm'
)

local function load_interpreter(cg)
    local module_uid = wasm.load(wasm_path, { interpreter = true })
    table.insert(cg.modules, module_uid)
    return module_uid
end

g.before_each(function(cg)
    cg.modules = {}
end)

g.after_each(function(cg)
    for _, module_uid in ipairs(cg.modules) do
        pcall(wasm.drop, module_uid)
    end
end)

g.test_shared_state_between_calls_and_instances = function(cg)
    local before = wasm.stats()
    local m1 = load_interpreter(cg)
    t.assert_equals(
        wasm.stats().current.interpreters,
        before.current.interpreters + 1
    )
    local m2 = load_interpreter(cg)
    t.assert_equals(
        wasm.stats().current.interpreters,
        before.current.interpreters + 1
    )

    local script = [[
counter = 0

def next():
    global counter
    counter += 1
    return counter
]]

    t.assert_equals(wasm.interpreter_exec(m1, script), true)

    t.assert_equals(wasm.interpreter_call(m1, 'next'), 1)
    t.assert_equals(wasm.interpreter_call(m2, 'next'), 2)

    t.assert_equals(wasm.drop(m1), true)

    t.assert_equals(wasm.interpreter_call(m2, 'next'), 3)

    t.assert_equals(wasm.drop(m2), true)
    t.assert_equals(
        wasm.stats().current.interpreters,
        before.current.interpreters
    )

    -- Remove dropped modules from automatic cleanup.
    cg.modules = {}

    local m3 = load_interpreter(cg)
    t.assert_equals(
        wasm.stats().current.interpreters,
        before.current.interpreters + 1
    )
    t.assert_equals(wasm.interpreter_exec(m3, script), true)
    t.assert_equals(wasm.interpreter_call(m3, 'next'), 1)
end

g.test_python_error_preserves_state = function(cg)
    local m = load_interpreter(cg)

    t.assert_equals(wasm.interpreter_exec(m, [[
counter = 0
def next_value():
    global counter
    counter += 1
    return counter

def fail():
    raise RuntimeError('expected failure')
]]), true)
    t.assert_equals(wasm.interpreter_call(m, 'next_value'), 1)

    local ok, err = pcall(wasm.interpreter_call, m, 'fail')
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), 'expected failure')

    t.assert_equals(wasm.interpreter_call(m, 'next_value'), 2)
end

g.test_large_payload_round_trip = function(cg)
    local m = load_interpreter(cg)
    t.assert_equals(wasm.interpreter_exec(m, [[
def echo(value):
    return value
]]), true)

    local payload = string.rep('x', 1024 * 1024)
    t.assert_equals(wasm.interpreter_call(m, 'echo', payload), payload)
end

g.test_rejects_different_config_for_shared_path = function(cg)
    load_interpreter(cg)

    local ok, err = pcall(wasm.load, wasm_path, {
        interpreter = true,
        memory_limit = 256 * 1024 * 1024,
    })
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), 'already loaded with a different config')
end

g.test_memory_limit_applies_during_interpreter_startup = function()
    local ok, err = pcall(wasm.load, wasm_path, {
        interpreter = true,
        memory_limit = 1024 * 1024,
    })
    t.assert_equals(ok, false)
    t.assert_str_contains(tostring(err), 'exceeds memory limits')
end

g.test_fuel_interrupts_call_and_interpreter_recovers = function(cg)
    local m = wasm.load(wasm_path, {
        interpreter = true,
        max_instructions = 100 * 1000 * 1000,
    })
    table.insert(cg.modules, m)

    t.assert_equals(wasm.interpreter_exec(m, [[
def spin():
    while True:
        pass
]]), true)

    local ok = pcall(wasm.interpreter_call, m, 'spin')
    t.assert_equals(ok, false)

    t.assert_equals(wasm.interpreter_exec(m, [[
def answer():
    return 42
]]), true)
    t.assert_equals(wasm.interpreter_call(m, 'answer'), 42)
end

g.test_concurrent_fibers_are_serialized = function(cg)
    local fiber = require('fiber')
    local m = load_interpreter(cg)
    local result = fiber.channel(20)

    t.assert_equals(wasm.interpreter_exec(m, [[
counter = 0
def next_value():
    global counter
    counter += 1
    return counter
]]), true)

    for _ = 1, 2 do
        fiber.new(function()
            for _ = 1, 10 do
                local value = wasm.interpreter_call(m, 'next_value')
                result:put(value)
            end
        end)
    end

    local values = {}
    for _ = 1, 20 do
        local value = result:get(10)
        t.assert(value ~= nil, 'interpreter call fiber timed out')
        t.assert_equals(type(value), 'number', tostring(value))
        table.insert(values, value)
    end
    table.sort(values)
    for i, value in ipairs(values) do
        t.assert_equals(value, i)
    end
end

g.test_tarantool_wasi_python_and_datetime = function(cg)
    local m = load_interpreter(cg)

    local script = [[
from wit_world.imports import metrics, monotonic_clock, session
import datetime
import hashlib

def tarantool_uptime():
    return metrics.uptime()

def wasi_now():
    return monotonic_clock.now()

def py_sha256():
    return hashlib.sha256(b"tarantool-wasm-rs").hexdigest()

def add(a, b):
    return a + b

def make_dt():
    return datetime.datetime(2024, 5, 6, 7, 8, 9, tzinfo=datetime.timezone.utc)

def make_naive_dt():
    return datetime.datetime(2024, 5, 6, 7, 8, 9)

def caller_session_id():
    return session.current().id
]]

    t.assert_equals(wasm.interpreter_exec(m, script), true)

    local uptime = wasm.interpreter_call(m, 'tarantool_uptime')
    t.assert_equals(type(uptime), 'number')
    t.assert_gt(uptime, 0)

    local now_ns = wasm.interpreter_call(m, 'wasi_now')
    t.assert_equals(type(now_ns), 'number')
    t.assert_gt(now_ns, 0)

    local digest = wasm.interpreter_call(m, 'py_sha256')
    t.assert_equals(digest, 'ba81ef2c684b5764f9331cee7e51d4e3193925ad0b28decf6af15f234b794287')

    t.assert_equals(wasm.interpreter_call(m, 'add', 10, 32), 42)

    local dt = wasm.interpreter_call(m, 'make_dt')
    t.assert_equals(dt.year, 2024)
    t.assert_equals(dt.month, 5)
    t.assert_equals(dt.day, 6)
    t.assert_equals(dt.hour, 7)
    t.assert_equals(dt.min, 8)
    t.assert_equals(dt.sec, 9)
    t.assert_str_contains(tostring(dt), '2024-05-06T07:08:09')

    local naive_dt = wasm.interpreter_call(m, 'make_naive_dt')
    t.assert_equals(naive_dt.year, 2024)
    t.assert_equals(naive_dt.month, 5)
    t.assert_equals(naive_dt.day, 6)
    t.assert_equals(naive_dt.tzoffset, 0)

    t.assert_equals(wasm.interpreter_call(m, 'caller_session_id'), box.session.id())
end
