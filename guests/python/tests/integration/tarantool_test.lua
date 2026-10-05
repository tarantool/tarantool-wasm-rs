local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')
local socket = require('socket')
local msgpack = require('msgpack')
local g = t.group('python_tarantool')
local component = fio.abspath('guests/python/tests/components/tarantool/tarantool.wasm')

g.before_all(function(cg)
    box.cfg{memtx_use_mvcc_engine = true}
    cg.users = box.schema.space.create('sdk_users')
    cg.users:create_index('primary')
    cg.users:create_index('name', {parts = {{2, 'string'}}})
    cg.ids = box.schema.space.create('sdk_ids')
    cg.ids:create_index('primary')
    cg.sequence = box.schema.sequence.create('sdk_sequence')
    cg.ids:replace({1, cg.sequence.id})
    cg.specials = box.schema.space.create('sdk_specials')
    cg.specials:format({{'id', 'unsigned'}, {'decimal', 'decimal'}, {'uuid', 'uuid'},
                       {'datetime', 'datetime'}, {'interval', 'interval'}, {'error', 'any'}})
    cg.specials:create_index('primary')
    cg.specials:create_index('decimal', {parts = {{2, 'decimal'}}})
    cg.specials:create_index('uuid', {parts = {{3, 'uuid'}}})
    cg.specials:create_index('datetime', {parts = {{4, 'datetime'}}})
    cg.specials:replace({1, require('decimal').new('-12.34'),
        require('uuid').fromstr('f6423bdf-b49e-4913-b361-0740c9702e4b'),
        require('datetime').new{timestamp = -1, nsec = 123456789, tzoffset = 180},
        require('datetime').interval.new{year = 1, month = 200, day = -77},
        box.error.new{code = 1, reason = 'extension test'}})
    cg.intervals = box.schema.space.create('sdk_intervals')
    cg.intervals:format({{'id', 'unsigned'}, {'value', 'interval'}})
    cg.intervals:create_index('primary')
    wasm.register_callback('sdk_echo', function(value) return value end)
    wasm.register_callback('sdk_failure', function() error('sdk callback failure') end)
    wasm.register_callback('sdk_binary', function(value) return require('varbinary').new(value) end)
    wasm.register_callback('sdk_plain_binary', function(value) return value end)
    wasm.register_callback('sdk_nulls', function() return {1, box.NULL, 3} end)
end)

g.before_each(function(cg)
    cg.users:truncate()
    cg.module = wasm.load(component)
end)

g.after_each(function(cg)
    if cg.module ~= nil then wasm.drop(cg.module) end
end)

g.after_all(function(cg)
    wasm.unregister_callback('sdk_echo')
    wasm.unregister_callback('sdk_failure')
    wasm.unregister_callback('sdk_binary')
    wasm.unregister_callback('sdk_plain_binary')
    wasm.unregister_callback('sdk_nulls')
    cg.users:drop()
    cg.ids:drop()
    cg.sequence:drop()
    cg.specials:drop()
    cg.intervals:drop()
end)

for _, name in ipairs({'crud', 'indexes', 'tuples', 'keys_formats', 'sequences',
                      'diagnostics', 'logging_api', 'messagepack', 'callbacks', 'raw_bindings',
                      'extensions_read', 'extensions_write', 'extensions_keys',
                      'extensions_callbacks', 'extensions_intervals'}) do
    g['test_' .. name] = function(cg)
        t.assert_equals(wasm.call(cg.module, 'check', name), 'ok')
    end
end

g.test_transactions = function(cg)
    t.assert(wasm.join(wasm.run(cg.module)))
    t.assert_equals(cg.users:get(11), {11, 'commit', 1})
    t.assert_equals(cg.users:get(12), nil)
    t.assert_equals(cg.users:get(13), nil)
    t.assert_equals(cg.users:get(14), {14, 'explicit commit', 1})
end

g.test_interpreter_sdk = function(cg)
    local interpreter = wasm.load(fio.abspath('guests/python/interpreter/adder.wasm'), {interpreter = true})
    local ok, err = pcall(function()
        t.assert(wasm.interpreter_exec(interpreter, [[
from twr_python_sdk import tarantool as box
import _tarantool
def sdk_call():
    users = box.space['sdk_users']
    users.replace((1, 'interpreter', 42))
    with box.lua_callback('sdk_echo') as echo:
        assert echo('hello') == 'hello'
    assert _tarantool.database.schema_version() == box.schema_version()
    return users.get(1)
]]))
        t.assert_equals(wasm.interpreter_call(interpreter, 'sdk_call'), {1, 'interpreter', 42})
    end)
    wasm.drop(interpreter)
    t.assert(ok, tostring(err))
end

g.test_callback_replacement = function()
    local interpreter = wasm.load(fio.abspath('guests/python/interpreter/adder.wasm'), {interpreter = true})
    wasm.register_callback('sdk_replace', function(value) return value + 1 end)
    local ok, err = pcall(function()
        t.assert(wasm.interpreter_exec(interpreter, [[
from twr_python_sdk import tarantool as box
callback = box.lua_callback('sdk_replace')
def invoke_callback(value):
    return callback(value)
def callback_is_missing():
    try:
        callback(1)
    except box.LuaCallbackError:
        return True
    return False
def close_callback():
    callback.close()
]]))
        t.assert_equals(wasm.interpreter_call(interpreter, 'invoke_callback', 10), 11)
        wasm.register_callback('sdk_replace', function(value) return value + 2 end)
        t.assert_equals(wasm.interpreter_call(interpreter, 'invoke_callback', 10), 12)
        wasm.unregister_callback('sdk_replace')
        t.assert_equals(wasm.interpreter_call(interpreter, 'callback_is_missing'), true)
        t.assert(wasm.interpreter_call(interpreter, 'close_callback') == nil)
    end)
    wasm.unregister_callback('sdk_replace')
    wasm.drop(interpreter)
    t.assert(ok, tostring(err))
end

g.test_interpreter_extensions = function(cg)
    local interpreter = wasm.load(fio.abspath('guests/python/interpreter/adder.wasm'), {interpreter = true})
    local ok, err = pcall(function()
        t.assert(wasm.interpreter_exec(interpreter, [[
from twr_python_sdk import tarantool as box
from decimal import Decimal
from uuid import UUID
def special_values(*values):
    assert isinstance(values[0], Decimal)
    assert isinstance(values[1], UUID)
    assert isinstance(values[2], box.Datetime)
    assert isinstance(values[3], box.Interval)
    assert isinstance(values[4], box.ErrorValue)
    assert tuple(values) == box.space['sdk_specials'].get(1)[1:]
    return values
]]))
        local row = cg.specials:get(1)
        local values = {row[2], row[3], row[4], row[5], row[6]}
        local returned = wasm.interpreter_call(interpreter, 'special_values', unpack(values))
        for i, value in ipairs(values) do
            -- Error object identity and metadata are not Lua scalar equality.
            t.assert_equals(msgpack.encode(returned[i]), msgpack.encode(value))
        end
    end)
    wasm.drop(interpreter)
    t.assert(ok, tostring(err))
end

g.test_sessions = function(cg)
    local path = fio.pathjoin(fio.tempdir(), 'sdk.sock')
    box.cfg{listen = path}
    local id
    local on_connect = function() id = box.session.id() end
    box.session.on_connect(on_connect)
    local client = assert(socket.tcp_connect('unix/', path))
    t.assert_equals(#client:read(128), 128)
    box.session.on_connect(nil, on_connect)
    cg.ids:replace({2, id})
    local received
    local watcher = box.watch('sdk_event', function(_, value) received = value end)
    local ok, result = pcall(wasm.call, cg.module, 'check', 'sessions')
    if ok then
        for sync = 77, 78 do
            local prefix = assert(client:read(5))
            local length = msgpack.decode(prefix)
            local packet = assert(client:read(length))
            local header, offset = msgpack.decode(packet)
            t.assert_equals(header, {[0] = 0, [1] = sync})
            if sync == 77 then
                t.assert_equals(msgpack.decode(packet, offset), {[0x30] = 'sdk'})
            else
                t.assert_equals(offset, #packet + 1)
            end
        end
        t.helpers.retrying({timeout = 2}, function() t.assert_equals(received, {answer = 42}) end)
    end
    watcher:unregister()
    client:close()
    box.cfg{listen = ''}
    fio.rmtree(fio.dirname(path))
    t.assert(ok, tostring(result))
    t.assert_equals(result, 'ok')
end
