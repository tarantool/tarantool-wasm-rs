local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')
local socket = require('socket')
local msgpack = require('msgpack')

local g = t.group('js_sdk')
local root = fio.abspath('guests/js/tests/components')

g.before_all(function(cg)
    box.cfg{memtx_use_mvcc_engine = true}

    cg.users = box.schema.space.create('js_users')
    cg.users:create_index('primary')
    cg.users:create_index('name', {parts = {{2, 'string'}}})

    cg.ids = box.schema.space.create('js_ids')
    cg.ids:create_index('primary')

    cg.seq = box.schema.sequence.create('js_seq')
    cg.ids:replace{1, cg.seq.id}

    cg.specials = box.schema.space.create('js_specials')
    cg.specials:format({
        {'id', 'unsigned'},
        {'decimal', 'decimal'},
        {'uuid', 'uuid'},
        {'datetime', 'datetime'},
        {'interval', 'interval'},
        {'error', 'any'},
    })
    cg.specials:create_index('primary')

    cg.intervals = box.schema.space.create('js_intervals')
    cg.intervals:format({{'id', 'unsigned'}, {'interval', 'interval'}})
    cg.intervals:create_index('primary')

    cg.bigints = box.schema.space.create('js_bigints')
    cg.bigints:create_index('primary')

    wasm.register_callback('js_echo', function(value)
        if value == nil or value == box.NULL then
            assert(rawequal(value, box.NULL))
        end

        return value
    end)

    wasm.register_callback('js_binary', function(value)
        assert(require('varbinary').is(value))

        return value
    end)

    wasm.register_callback('js_fail', function()
        error('JS callback failure')
    end)
end)

g.before_each(function(cg)
    cg.users:truncate()

    cg.base = wasm.load(fio.pathjoin(root, 'base/sdk.wasm'))
    cg.extras = wasm.load(fio.pathjoin(root, 'extras/sdk.wasm'))
end)

g.after_each(function(cg)
    if cg.base ~= nil then
        wasm.drop(cg.base)
    end

    if cg.extras ~= nil then
        wasm.drop(cg.extras)
    end
end)

g.after_all(function(cg)
    for _, name in ipairs({'js_echo', 'js_binary', 'js_fail', 'js_replace'}) do
        wasm.unregister_callback(name)
    end

    cg.users:drop()
    cg.ids:drop()
    cg.seq:drop()
    cg.specials:drop()
    cg.intervals:drop()
    cg.bigints:drop()
end)

for _, name in ipairs({
    'crud',
    'indexes',
    'tuples',
    'keys',
    'sequences',
    'diagnostics',
    'logging',
    'messagepack',
    'extensions',
}) do
    g['test_' .. name] = function(cg)
        t.assert_equals(wasm.call(cg.base, 'check', name), 'ok')
    end
end

for _, name in ipairs({'lua-callback', 'zlib'}) do
    g['test_' .. name:gsub('-', '_')] = function(cg)
        t.assert_equals(wasm.call(cg.extras, 'check', name), 'ok')
    end
end

g.test_schema_changes = function(cg)
    local before = wasm.call(cg.base, 'schema')
    local old_id = cg.users.id

    cg.users:drop()

    cg.users = box.schema.space.create('js_users', {id = old_id + 1000})
    cg.users:create_index('primary', {id = 0})
    cg.users:create_index('name', {id = 3, parts = {{2, 'string'}}})

    t.assert(wasm.call(cg.base, 'schema') > before)

    -- This executes named secondary-index lookups after changing IDs.
    t.assert_equals(wasm.call(cg.base, 'check', 'crud'), 'ok')
end

g.test_transactions = function(cg)
    local directory = fio.tempdir()
    local stderr = fio.pathjoin(directory, 'stderr')

    local ok, result = pcall(function()
        return wasm.join(wasm.run(cg.base, {stderr = stderr}))
    end)

    local file = io.open(stderr, 'r')
    local diagnostic = file and file:read('*a') or ''
    if file ~= nil then
        file:close()
    end

    fio.rmtree(directory)

    t.assert(ok, tostring(result) .. ' ' .. diagnostic)
    t.assert(result)
    t.assert_equals(cg.users:get(11), {11, 'commit', 1})
    t.assert_equals(cg.users:get(12), nil)
    t.assert_equals(cg.users:get(13), nil)
    t.assert_equals(cg.users:get(14), {14, 'explicit', 1})

    -- The SDK preserves the host's wasm.call transaction restriction.
    t.assert_equals(wasm.call(cg.base, 'check', 'transactionRejected'), 'ok')
end

g.test_callback_replacement = function(cg)
    wasm.register_callback('js_replace', function(value)
        return value + 1
    end)

    t.assert_equals(wasm.call(cg.extras, 'invoke', 10), 11)

    wasm.register_callback('js_replace', function(value)
        return value + 2
    end)

    t.assert_equals(wasm.call(cg.extras, 'invoke', 10), 12)
    wasm.unregister_callback('js_replace')

    t.assert_not(pcall(wasm.call, cg.extras, 'invoke', 10))
    wasm.call(cg.extras, 'close')
    wasm.call(cg.extras, 'close')
end

g.test_sessions = function(cg)
    local directory = fio.tempdir()
    local path = fio.pathjoin(directory, 'js.sock')
    box.cfg{listen = path}
    local id
    local on_connect = function()
        id = box.session.id()
    end
    box.session.on_connect(on_connect)
    local client = assert(socket.tcp_connect('unix/', path))

    t.assert_equals(#client:read(128), 128)
    box.session.on_connect(nil, on_connect)
    cg.ids:replace{2, id}

    local received

    local watcher = box.watch('js_event', function(_, value)
        received = value
    end)

    local ok, result = pcall(wasm.call, cg.base, 'check', 'sessions')
    if ok then
        for sync = 77, 78 do
            local length = msgpack.decode(assert(client:read(5)))
            local packet = assert(client:read(length))
            local header, offset = msgpack.decode(packet)

            t.assert_equals(header, {[0] = 0, [1] = sync})
            if sync == 77 then
                t.assert_equals(
                    msgpack.decode(packet, offset),
                    {[0x30] = 'sdk'}
                )
            else
                t.assert_equals(offset, #packet + 1)
            end
        end
        t.helpers.retrying({timeout = 2}, function()
            t.assert_equals(received, {answer = 42})
        end)
    end

    watcher:unregister()
    client:close()
    box.cfg{listen = ''}

    fio.rmtree(directory)

    t.assert(ok, tostring(result))
    t.assert_equals(result, 'ok')
end
