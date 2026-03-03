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
    local m1 = load_interpreter(cg)
    local m2 = load_interpreter(cg)

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

    -- Remove dropped modules from automatic cleanup.
    cg.modules = {}

    local m3 = load_interpreter(cg)
    t.assert_equals(wasm.interpreter_exec(m3, script), true)
    t.assert_equals(wasm.interpreter_call(m3, 'next'), 1)
end

g.test_tarantool_wasi_python_and_datetime = function(cg)
    local m = load_interpreter(cg)

    local script = [[
from wit_world.imports import metrics, monotonic_clock
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
end
