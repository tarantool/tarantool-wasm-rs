-- Isolated process: a blocked TX thread cannot run an in-process timeout.
local fio = require('fio')
local fiber = require('fiber')
local scenario = assert(os.getenv('WASM_TXN_SCENARIO'))
local engine = assert(os.getenv('WASM_TXN_ENGINE'))
assert(engine == 'memtx' or engine == 'vinyl')
box.cfg{
    memtx_dir = assert(os.getenv('WASM_TXN_WORKDIR')),
    wal_dir = os.getenv('WASM_TXN_WORKDIR'),
    vinyl_dir = os.getenv('WASM_TXN_WORKDIR'),
    log = fio.pathjoin(os.getenv('WASM_TXN_WORKDIR'), 'tarantool.log'),
    -- Vinyl must work independently of the memtx transaction manager.
    memtx_use_mvcc_engine = engine == 'memtx' and scenario ~= 'non_mvcc',
}
box.ctl.set_on_shutdown_timeout(10)
-- Registered before the runtime so this check runs after its shutdown hook.
local check_shutdown
if scenario == 'shutdown' or scenario == 'shutdown_cpu' then
    box.ctl.on_shutdown(function() check_shutdown() end)
end
local wasm = require('wasm')
local component = fio.pathjoin(assert(os.getenv('WASM_REPO')),
    'test/tarantool_bindings/wasm_component_test/bindings_pipe_txn_test/python/adder.wasm')
local m
if scenario ~= 'run' then m = wasm.load(component) end
local space = box.schema.space.create('pipe_txn', {engine = engine})
assert(space.engine == engine)
if engine == 'vinyl' then assert(not box.cfg.memtx_use_mvcc_engine) end
space:create_index('primary')

local function response(p, timeout)
    local data = ''
    local deadline = fiber.clock() + timeout / 1000
    while not data:find('\n', 1, true) do
        local remaining = math.max(1, math.floor((deadline - fiber.clock()) * 1000))
        assert(fiber.clock() < deadline, 'timed out waiting for response: ' .. data)
        local chunk, eof = p:read({stream = 'stdout', max_bytes = 4096, timeout_ms = remaining})
        data = data .. (chunk or '')
        assert(not eof or data:find('\n', 1, true), 'unexpected EOF: ' .. data)
    end
    return data
end

local function start()
    local p = wasm.pipe.spawn(m, nil, {capacity = 4096})
    assert(response(p, 60000) == 'ready\n')
    return p
end

local function command(p, input, expected)
    p:write(input .. '\n', {timeout_ms = 5000})
    local result = response(p, 5000)
    if expected then
        assert(result == expected .. '\n', input .. ': ' .. result)
    end
    return result
end

local function finish(p)
    p:close_stdin()
    assert(p:join({timeout_ms = 10000}))
end

local function no_transaction_fibers()
    for _, info in pairs(fiber.info()) do
        if info.name == 'wasm-tx' then return false end
    end
    return true
end

local function wait_cleanup()
    local deadline = fiber.clock() + 5
    while not no_transaction_fibers() and fiber.clock() < deadline do fiber.sleep(0.01) end
    assert(no_transaction_fibers(), 'transaction fiber leaked')
end

if scenario == 'space_lookup_conflict' then
    local p = start()
    command(p, 'BEGIN', 'begin')
    command(p, 'SPACE_LOOKUP', 'space-found')
    command(p, 'INSERT 1', 'insert')
    space:create_index('secondary', {parts = {1, 'unsigned'}, unique = false})
    command(p, 'SPACE_LOOKUP', 'error:SPACE_LOOKUP:97')
    command(p, 'ROLLBACK', 'rollback')
    finish(p)
    wait_cleanup()
elseif scenario == 'semantics' then
    local p = start()
    command(p, 'BEGIN', 'begin')
    local id = command(p, 'ID')
    assert(id:match('^id=%d+\n$'))
    command(p, 'INSERT 1', 'insert')
    assert(space:get{1} == nil, 'uncommitted tuple became visible')
    command(p, 'BEGIN', 'error:BEGIN')
    assert(command(p, 'ID') == id, 'duplicate begin discarded the original transaction')
    command(p, 'COMMIT', 'commit')
    assert(space:get{1}, 'commit lost the tuple')
    if engine == 'vinyl' then
        -- Exercise a vinyl dump before subsequent reads and uniqueness checks.
        box.snapshot()
        assert(space:get{1}, 'vinyl snapshot lost the committed tuple')
    end

    command(p, 'BEGIN', 'begin')
    command(p, 'INSERT 2', 'insert')
    command(p, 'ROLLBACK', 'rollback')
    assert(space:get{2} == nil, 'rollback kept the tuple')

    -- A conflicting statement must fail without moving subsequent statements
    -- outside the transaction.
    command(p, 'BEGIN', 'begin')
    command(p, 'INSERT 1', 'error:INSERT')
    command(p, 'INSERT 3', 'insert')
    command(p, 'ROLLBACK', 'rollback')
    assert(space:get{3} == nil)
    finish(p)
    wait_cleanup()

    -- EOF, trap, and cancellation must all roll back and release the owner.
    for i, ending in ipairs({'eof', 'trap', 'cancel'}) do
        p = start()
        command(p, 'BEGIN', 'begin')
        command(p, 'INSERT ' .. (i + 3), 'insert')
        if ending == 'trap' then
            p:write('TRAP\n', {timeout_ms = 5000})
            p:close_stdin()
            local ok, err = pcall(function() p:join({timeout_ms = 10000}) end)
            assert(not ok and not tostring(err):match('canceled'),
                   'guest trap was misclassified: ' .. tostring(err))
        elseif ending == 'cancel' then
            assert(p:cancel())
            pcall(function() p:join({timeout_ms = 10000}) end)
        else
            finish(p)
        end
        wait_cleanup()
        assert(space:get{i + 3} == nil, ending .. ' kept uncommitted data')
    end
elseif scenario == 'cancel_isolated' then
    -- First check independent guests with no Tarantool transaction.
    local a = start()
    local b = start()
    a:write('LOOP\n', {timeout_ms = 5000})
    assert(response(a, 5000) == 'looping\n')
    fiber.sleep(0.05) -- Let the worker enter the CPU loop.
    b:write('BUSY\n', {timeout_ms = 5000})
    assert(response(b, 5000) == 'busy\n')
    local started = fiber.clock()
    assert(a:cancel())
    local ok, err = pcall(function() a:join({timeout_ms = 5000}) end)
    assert(not ok and tostring(err):match('canceled'), tostring(err))
    assert(fiber.clock() - started < 5, 'CPU-bound cancellation missed deadline')
    assert(response(b, 5000) == 'done\n', 'B was interrupted by A cancellation')
    assert(a:close({timeout_ms = 1000}))
    finish(b)

    -- Repeat with both transaction owners open and check independent outcomes.
    a = start()
    b = start()
    command(a, 'BEGIN', 'begin')
    command(b, 'BEGIN', 'begin')
    command(a, 'INSERT 20', 'insert')
    command(b, 'INSERT 21', 'insert')
    a:write('LOOP\n', {timeout_ms = 5000})
    assert(response(a, 5000) == 'looping\n')
    fiber.sleep(0.05) -- Let the worker enter the CPU loop.
    b:write('BUSY\n', {timeout_ms = 5000})
    assert(response(b, 5000) == 'busy\n')
    local started = fiber.clock()
    assert(a:cancel())
    assert(a:cancel())
    local ok, err = pcall(function() a:join({timeout_ms = 5000}) end)
    assert(not ok and tostring(err):match('canceled'), tostring(err))
    assert(fiber.clock() - started < 5, 'CPU-bound cancellation missed deadline')
    ok, err = pcall(function() a:join({timeout_ms = 1000}) end)
    assert(not ok and tostring(err):match('canceled'), tostring(err))
    assert(a:close({timeout_ms = 1000}))
    assert(a:close({timeout_ms = 1000}))
    assert(response(b, 5000) == 'done\n', 'B was interrupted by A cancellation')
    command(b, 'COMMIT', 'commit')
    finish(b)
    wait_cleanup()
    assert(space:get{20} == nil, 'A transaction was not rolled back')
    assert(space:get{21}, 'B transaction did not commit')
elseif scenario == 'cancel_streams' then
    local a = start()
    assert(a:cancel()) -- Guest is blocked reading stdin.
    local ok, err = pcall(function() a:join({timeout_ms = 5000}) end)
    assert(not ok and tostring(err):match('canceled'), tostring(err))
    assert(a:close({timeout_ms = 1000}))
    a = start()
    a:write('SPAM\n', {timeout_ms = 5000})
    fiber.sleep(0.1) -- Fill stdout without draining it.
    assert(a:cancel())
    ok, err = pcall(function() a:join({timeout_ms = 5000}) end)
    assert(not ok and tostring(err):match('canceled'), tostring(err))
    assert(a:close({timeout_ms = 1000}))
    -- Hold a real Tarantool host call in an on_replace trigger. The parent
    -- receives a barrier before canceling, then lets the TX fiber finish.
    a = start()
    command(a, 'BEGIN', 'begin')
    local entered = fiber.channel(1)
    local release = fiber.channel(1)
    local trigger = function()
        assert(entered:put(true, 5))
        assert(release:get(5), 'host call release timed out')
    end
    space:on_replace(trigger)
    a:write('INSERT 30\n', {timeout_ms = 5000})
    assert(entered:get(5), 'guest did not enter the host call')
    assert(a:cancel())
    assert(release:put(true, 5))
    ok, err = pcall(function() a:join({timeout_ms = 5000}) end)
    assert(not ok and tostring(err):match('canceled'), tostring(err))
    assert(a:close({timeout_ms = 1000}))
    space:on_replace(nil, trigger)
    wait_cleanup()
    assert(space:get{30} == nil, 'host-call cancellation did not roll back')
elseif scenario == 'cancel_race' then
    local a = start()
    a:write('FINISH\n', {timeout_ms = 5000})
    assert(a:cancel())
    local ok, err = pcall(function() a:join({timeout_ms = 5000}) end)
    assert(ok or tostring(err):match('canceled'), tostring(err))
    local again, again_err = pcall(function() a:join({timeout_ms = 1000}) end)
    assert(again == ok, 'join outcome changed after completion')
    if not ok then
        assert(tostring(again_err):match('canceled'), tostring(again_err))
    end
    assert(a:close({timeout_ms = 1000}))
    assert(a:close({timeout_ms = 1000}))
    -- Once completion has won, a late cancel cannot rewrite its result.
    a = start()
    command(a, 'FINISH', 'finished')
    assert(a:join({timeout_ms = 5000}))
    assert(a:cancel())
    assert(a:join({timeout_ms = 1000}))
    assert(a:close({timeout_ms = 1000}))
    wait_cleanup()
elseif scenario == 'shutdown_cpu' then
    _G.txn_shutdown_pipe = start()
    command(_G.txn_shutdown_pipe, 'BEGIN', 'begin')
    command(_G.txn_shutdown_pipe, 'INSERT 22', 'insert')
    _G.txn_shutdown_pipe:write('LOOP\n', {timeout_ms = 5000})
    assert(response(_G.txn_shutdown_pipe, 5000) == 'looping\n')
    fiber.sleep(0.05) -- Let the worker enter the CPU loop.
    check_shutdown = function()
        wait_cleanup()
        assert(space:get{22} == nil, 'shutdown kept CPU-loop transaction')
        print('ok')
        io.stdout:flush()
    end
    os.exit(0)
elseif scenario == 'run' then
    local input = fio.pathjoin(os.getenv('WASM_TXN_WORKDIR'), 'input')
    local output = fio.pathjoin(os.getenv('WASM_TXN_WORKDIR'), 'output')
    local file = assert(io.open(input, 'w'))
    file:write('BEGIN\nINSERT 1\nCOMMIT\nBEGIN\nINSERT 2\nROLLBACK\n')
    file:close()
    local run_module = wasm.load(component, {stdin = input, stdout = output})
    assert(wasm.join(wasm.run(run_module)))
    wait_cleanup()
    file = assert(io.open(output, 'r'))
    local result = file:read('*a')
    file:close()
    assert(result == 'ready\nbegin\ninsert\ncommit\nbegin\ninsert\nrollback\n', result)
    assert(space:get{1}, 'wasm.run commit lost the tuple')
    assert(space:get{2} == nil, 'wasm.run rollback kept the tuple')
elseif scenario == 'pool' then
    -- Occupy more transaction owners than the default four eio workers.
    -- fio also needs eio and must remain available while all owners are idle.
    local pipes = {}
    for i = 1, 8 do
        local p = start()
        pipes[i] = p
        command(p, 'BEGIN', 'begin')
        command(p, 'INSERT ' .. i, 'insert')
    end
    local ch = fiber.channel(1)
    fiber.create(function()
        local file = assert(fio.open(fio.pathjoin(os.getenv('WASM_TXN_WORKDIR'), 'probe'),
                                    {'O_CREAT', 'O_WRONLY'}, tonumber('600', 8)))
        file:close()
        ch:put(true)
    end)
    assert(ch:get(5), 'idle transactions exhausted the eio pool')
    for i, p in ipairs(pipes) do
        command(p, 'COMMIT', 'commit')
        finish(p)
        assert(space:get{i})
    end
    wait_cleanup()
elseif scenario == 'shutdown' then
    _G.txn_shutdown_pipe = start()
    command(_G.txn_shutdown_pipe, 'BEGIN', 'begin')
    command(_G.txn_shutdown_pipe, 'INSERT 1', 'insert')
    check_shutdown = function()
        local ok, err = pcall(wasm.load, component)
        assert(not ok and tostring(err):match('WASM runtime is stopp'))
        wait_cleanup()
        assert(space:get{1} == nil, 'shutdown kept uncommitted data')
        print('ok')
        io.stdout:flush()
    end
    os.exit(0)
elseif scenario == 'non_mvcc' then
    -- Cooperative host calls obey Tarantool's yield restrictions.
    local p = start()
    command(p, 'BEGIN', 'begin')
    command(p, 'INSERT 1', 'insert')
    command(p, 'COMMIT', 'error:COMMIT')
    command(p, 'ID', 'id=None')
    assert(space:get{1} == nil)
    -- A failed commit must leave the store usable.
    command(p, 'BEGIN', 'begin')
    command(p, 'ROLLBACK', 'rollback')
    finish(p)
    wait_cleanup()
else
    error('unknown scenario: ' .. scenario)
end

print('ok')
io.stdout:flush()
os.exit(0)
