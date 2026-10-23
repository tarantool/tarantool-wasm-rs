local t = require('luatest')
local datetime = require('datetime')
local decimal = require('decimal')
local msgpack = require('msgpack')
local uuid = require('uuid')
local varbinary = require('varbinary')
local wasm = require('wasm')

local g = t.group('rust_sdk')

g.before_all(function(cg)
    box.cfg{}

    cg.space = box.schema.space.create('rust_users')
    cg.space:create_index('primary')

    cg.specials = box.schema.space.create('rust_specials')
    cg.specials:format({
        {'id', 'unsigned'},
        {'decimal', 'decimal'},
        {'uuid', 'uuid'},
        {'datetime', 'datetime'},
        {'interval', 'interval'},
        {'error', 'any'},
        {'binary', 'varbinary'},
        {'nil', 'any', is_nullable = true},
        {'bigint', 'unsigned'},
    })
    cg.specials:create_index('primary')

    cg.component = wasm.load(assert(os.getenv('TWR_RUST_COMPONENT')))
end)

g.after_all(function(cg)
    if cg.component ~= nil then
        wasm.drop(cg.component)
    end
    if cg.space ~= nil then
        cg.space:drop()
    end
    if cg.specials ~= nil then
        cg.specials:drop()
    end
end)

g.test_crud_and_ownership = function(cg)
    t.assert_equals(wasm.call(cg.component, 'check'), 'ok')
end

g.test_persistent_session = function(cg)
    local session = wasm.open_session(cg.component)
    local ok, err = pcall(function()
        for _ = 1, 8 do
            t.assert_equals(wasm.session_call(session, 'check'), 'ok')
        end
    end)

    wasm.close_session(session)
    if not ok then
        error(err)
    end
end

g.test_msgpack_extensions = function(cg)
    local values = {
        decimal.new('-12345678901234567890.123456789012345678'),
        uuid.fromstr('00112233-4455-6677-8899-aabbccddeeff'),
        datetime.new({timestamp = -1, nsec = 999999999, tzoffset = 180}),
        datetime.interval.new({month = -2, day = 3, adjust = 'last'}),
        box.error.new({code = box.error.ILLEGAL_PARAMS, reason = 'SDK test'}),
        varbinary.new('\0\255'),
        box.NULL,
        18446744073709551615ULL,
    }

    for _, value in ipairs(values) do
        local data = msgpack.encode(value)
        local bytes = {data:byte(1, #data)}
        local result = wasm.call(cg.component, 'roundtrip', bytes)

        -- Compare canonical wire values; box.error identity is not comparable.
        t.assert_equals(string.char(unpack(result)), data)
    end

    table.insert(values, 1, 1)
    local data = msgpack.encode(values)
    local bytes = {data:byte(1, #data)}
    local result = wasm.call(cg.component, 'exchange', bytes)

    t.assert_equals(string.char(unpack(result)), data)
    t.assert_equals(msgpack.encode(cg.specials:get(1):totable()), data)
end
