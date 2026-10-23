local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')
local msgpack = require('msgpack')

local g = t.group('csharp_sdk')
local components = assert(os.getenv('TWR_CSHARP_COMPONENTS'))

local function enc(value)
    local data = msgpack.encode(value)
    return {data:byte(1, #data)}
end

local function dec(bytes)
    return msgpack.decode(string.char(unpack(bytes)))
end

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

g.before_all(function(cg)
    box.cfg{memtx_use_mvcc_engine = true}

    cg.users = box.schema.space.create('csharp_users')
    cg.users:format({
        {'id', 'unsigned'},
        {'name', 'string'},
        {'score', 'integer'},
    })
    cg.users:create_index('primary')
    cg.users:create_index('name', {parts = {{2, 'string'}}})

    cg.specials = box.schema.space.create('csharp_specials')
    cg.specials:format({
        {'id', 'unsigned'},
        {'decimal', 'decimal'},
        {'uuid', 'uuid'},
        {'datetime', 'datetime'},
        {'interval', 'interval'},
        {'error', 'any'},
        {'binary', 'varbinary'},
    })
    cg.specials:create_index('primary')

    cg.bigints = box.schema.space.create('csharp_bigints')
    cg.bigints:create_index('primary')

    cg.base = wasm.load(fio.pathjoin(components, 'base.wasm'))
    cg.alternate = wasm.load(fio.pathjoin(components, 'alternate.wasm'))
end)

g.before_each(function(cg)
    cg.users:truncate()
end)

g.after_all(function(cg)
    if cg.base ~= nil then
        wasm.drop(cg.base)
    end

    if cg.alternate ~= nil then
        wasm.drop(cg.alternate)
    end

    cg.users:drop()
    cg.specials:drop()
    cg.bigints:drop()
end)

for _, name in ipairs({
    'crud',
    'indexes',
    'tuples',
    'errors',
    'extensions',
    'invalid',
}) do
    g['test_' .. name] = function(cg)
        t.assert_equals(wasm.call(cg.base, 'check', name), 'ok')
    end
end

g.test_independent_world_and_packaged_assembly = function(cg)
    t.assert_equals(
        wasm.call(cg.alternate, 'create', 42, 'другой world 🙂'),
        'другой world 🙂'
    )
    t.assert_equals(
        cg.users:get(42):totable(),
        {42, 'другой world 🙂', 0}
    )
end

g.test_lua_to_csharp_read = function(cg)
    cg.users:insert({99, 'Lua', 1})

    t.assert_equals(wasm.call(cg.base, 'check', 'lua-read'), 'ok')
    t.assert_equals(cg.users:get(99):totable(), {99, 'Lua', 1})
    t.assert_equals(wasm.call(cg.base, 'space-id'), cg.users.id)

    local value = {
        box.NULL,
        require('varbinary').new('\0\255'),
        {a = 1},
        'Привет\0🙂',
    }

    t.assert_equals(
        comparable(dec(wasm.call(cg.base, 'roundtrip', enc(value)))),
        comparable(value)
    )
end

g.test_host_extension_exchange = function(cg)
    t.assert_equals(wasm.call(cg.base, 'check', 'extensions'), 'ok')

    local uuid = require('uuid')
    local decimal = require('decimal')
    local datetime = require('datetime')
    local row = cg.specials:get(1):totable()

    t.assert_equals(tostring(row[2]), '1234567890123456789012345678.9012345678')
    t.assert_equals(
        row[3],
        uuid.fromstr('00112233-4455-6677-8899-aabbccddeeff')
    )
    t.assert_equals(row[4].nsec, 999999999)
    t.assert_equals(row[4].tzoffset, 180)

    local lua_value = {
        2,
        decimal.new('-1234567890123456789012345678.9012345678'),
        uuid.fromstr('ffffffff-ffff-ffff-ffff-ffffffffffff'),
        datetime.new{timestamp = -1, nsec = 123456789, tzoffset = -180},
        datetime.interval.new{year = -1, day = 4, adjust = 'last'},
        box.error.new{code = 42, reason = 'Lua error value'},
        require('varbinary').new('\0\255'),
    }
    cg.specials:replace(lua_value)

    for _, value in ipairs({row, cg.specials:get(2):totable()}) do
        local out = dec(wasm.call(cg.base, 'roundtrip', enc(value)))

        t.assert_equals(comparable(out), comparable(value))
    end
end

g.test_integer_precision = function(cg)
    t.assert_equals(wasm.call(cg.base, 'check', 'extensions'), 'ok')

    local row = cg.bigints:get(1):totable()
    local ffi = require('ffi')

    t.assert_equals(row[2], ffi.cast('uint64_t', -1))
    t.assert_equals(row[3], -9223372036854775807LL - 1LL)
    t.assert_equals(row[4], 9007199254740992ULL)
    t.assert_equals(dec(wasm.call(cg.base, 'roundtrip', enc(row))), row)
end

g.test_schema_changes_in_persistent_session = function(cg)
    local session = wasm.open_session(cg.base)

    local ok, err = pcall(function()
        local old = wasm.session_call(session, 'space-id')

        t.assert_equals(
            wasm.session_call(session, 'index-id'),
            cg.users.index.name.id
        )
        cg.users:drop()

        cg.users = box.schema.space.create('csharp_users', {id = old + 1000})
        cg.users:create_index('primary')
        cg.users:create_index('name', {id = 5, parts = {{2, 'string'}}})

        t.assert_equals(wasm.session_call(session, 'space-id'), cg.users.id)
        t.assert_equals(wasm.session_call(session, 'index-id'), 5)
    end)

    wasm.close_session(session)
    if not ok then
        error(err)
    end
end

g.test_session_resource_cleanup = function(cg)
    local session = wasm.open_session(cg.base)

    local ok, err = pcall(function()
        for _ = 1, 8 do
            t.assert_equals(wasm.session_call(session, 'check', 'tuples'), 'ok')
            cg.users:truncate()

            t.assert_equals(
                wasm.session_call(session, 'check', 'indexes'),
                'ok'
            )
        end
    end)

    wasm.close_session(session)
    if not ok then
        error(err)
    end
end

g.test_exception_record = function(cg)
    cg.users:insert({10, 'original', 0})

    local error_value = dec(wasm.call(cg.base, 'error-details'))

    t.assert_equals(error_value[1], box.error.TUPLE_FOUND)
    t.assert_str_contains(error_value[2], 'csharp_users')
    t.assert_type(error_value[3], 'string')
    t.assert_equals(cg.users:get(10):totable(), {10, 'original', 0})
end
