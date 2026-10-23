local t = require('luatest')
local datetime = require('datetime')
local decimal = require('decimal')
local fio = require('fio')
local msgpack = require('msgpack')
local uuid = require('uuid')
local varbinary = require('varbinary')
local wasm = require('wasm')

local g = t.group('sdk_compatibility')
local languages = {'rust', 'csharp', 'go', 'js', 'python'}
local components = {
    rust = 'guests/rust/tests/component/target/wasm32-wasip2/release/' ..
        'rust_sdk_test.wasm',
    csharp = 'guests/csharp/dist/components/base.wasm',
    go = 'guests/go/tests/components/base/sdk.wasm',
    js = 'guests/js/tests/components/base/sdk.wasm',
    python = 'guests/python/interpreter/adder.wasm',
}

local function comparable(value)
    if box.error.is(value) then
        return value:unpack()
    end
    if type(value) ~= 'table' then
        return value
    end

    local result = {}

    for key, item in pairs(value) do
        result[key] = comparable(item)
    end

    return result
end

local function roundtrip(cg, language, data)
    local component = cg.modules[language]
    if language == 'python' then
        return wasm.interpreter_call(
            component, 'sdk_roundtrip', varbinary.new(data)
        )
    end

    local result = wasm.call(component, 'roundtrip', {data:byte(1, -1)})

    return string.char(unpack(result))
end

g.before_all(function(cg)
    box.cfg{}

    cg.spaces = {}
    cg.modules = {}

    -- Existing fixtures resolve these names during module initialization.
    for _, name in ipairs({'csharp_users', 'go_users', 'js_users'}) do
        local space = box.schema.space.create(name)
        table.insert(cg.spaces, space)

        space:create_index('primary')
        space:create_index('name', {parts = {{2, 'string'}}})
    end

    cg.exchange = box.schema.space.create('sdk_compatibility')
    table.insert(cg.spaces, cg.exchange)
    cg.exchange:create_index('primary')

    for _, language in ipairs(languages) do
        cg.modules[language] = wasm.load(
            fio.abspath(components[language]),
            {interpreter = language == 'python'}
        )
    end

    t.assert(wasm.interpreter_exec(cg.modules.python, [[
from twr_python_sdk import tarantool as box

def sdk_roundtrip(data):
    return box.msgpack.pack(box.msgpack.unpack(data))
]]))

    local error_value = box.error.new{
        code = 42,
        reason = 'SDK compatibility',
        object_name = 'shared tuple',
    }

    cg.rows = {
        {1, box.NULL, false, true, -33, 128, 1.5, 'Привет 🙂'},
        {1, 18446744073709551615ULL, -9223372036854775808LL},
        {1, 9007199254740993ULL, -9007199254740993LL},
        {1, varbinary.new(''), varbinary.new('\0\255\1\254')},
        {
            1,
            {box.NULL, false, {
                text = 'nested',
                binary = varbinary.new('\0\255'),
            }},
            {[7] = 'integer key', key = 'value'},
        },
        {1, decimal.new('-12345678901234567890.123456789012345678')},
        {1, uuid.fromstr('00112233-4455-6677-8899-aabbccddeeff')},
        {1, datetime.new{timestamp = -1, nsec = 999999999, tzoffset = 180}},
        {1, datetime.interval.new{
            year = 2, month = -3, day = -4, adjust = 'last',
        }},
        {1, datetime.interval.new{month = 2, day = -1, adjust = 'none'}},
        {1, datetime.interval.new{month = -2, day = 3, adjust = 'excess'}},
        {1, error_value, {error = error_value}},
    }
end)

g.after_all(function(cg)
    for _, language in ipairs(languages) do
        if cg.modules[language] ~= nil then
            wasm.drop(cg.modules[language])
        end
    end

    for _, space in ipairs(cg.spaces) do
        space:drop()
    end
end)

for _, producer in ipairs(languages) do
    for _, consumer in ipairs(languages) do
        g['test_' .. producer .. '_to_' .. consumer] = function(cg)
            for case, row in ipairs(cg.rows) do
                local label = producer .. ' -> ' .. consumer ..
                    ', case ' .. case
                local encoded = msgpack.encode(row)
                local produced = roundtrip(cg, producer, encoded)

                t.assert_equals(
                    comparable(msgpack.decode(produced)), comparable(row),
                    label .. ': producer'
                )

                cg.exchange:replace(msgpack.decode(produced))
                local stored = msgpack.encode(cg.exchange:get(1):totable())
                local consumed = roundtrip(cg, consumer, stored)

                t.assert_equals(
                    comparable(msgpack.decode(consumed)), comparable(row),
                    label .. ': consumer'
                )
            end

            -- Preserve empty array/map tags as well as their decoded values.
            local empty_containers = string.char(0x93, 1, 0x90, 0x80)
            local produced = roundtrip(cg, producer, empty_containers)
            local consumed = roundtrip(cg, consumer, produced)

            t.assert_equals(produced, empty_containers)
            t.assert_equals(consumed, empty_containers)
        end
    end
end
