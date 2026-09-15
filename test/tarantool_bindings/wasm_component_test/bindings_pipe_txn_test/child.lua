-- Child half of `tarantool_bindings_pipe_txn_test.lua`.
--
-- It drives a transaction whose statements arrive as separate, time-separated
-- pipe messages. This is the scenario that used to deadlock the whole Tarantool
-- instance: once `transaction.begin()` migrated the guest onto the TX thread,
-- the guest's wait for the next message parked TX, so the host fiber could
-- never write it. Running the protocol in a child process lets the parent test
-- impose a wall-clock timeout and fail instead of hanging the test run.

local fio = require('fio')
local fiber = require('fiber')

local workdir = fio.tempdir()
box.cfg{
    memtx_dir = workdir,
    wal_dir = workdir,
    vinyl_dir = workdir,
    log = fio.pathjoin(workdir, 'tarantool.log'),
    log_level = 5,
    -- Required so the transaction survives the fiber yields between messages.
    memtx_use_mvcc_engine = true,
}
box.ctl.set_on_shutdown_timeout(10)

local root = assert(os.getenv('WASM_REPO'))
local wasm = require('wasm')

local component = fio.pathjoin(
    root,
    'test/tarantool_bindings/wasm_component_test/bindings_pipe_txn_test/python/adder.wasm'
)

local function fail(message)
    io.stderr:write(('pipe txn child: %s\n'):format(message))
    io.stderr:flush()
    os.exit(1)
end

local m = wasm.load(component)
local p = wasm.pipe.spawn(m, nil, { capacity = 4096 })

local function read_until(needle, timeout_ms)
    local deadline = fiber.clock() + timeout_ms / 1000
    local acc = ''
    while fiber.clock() < deadline do
        local remaining = math.max(1, math.floor((deadline - fiber.clock()) * 1000))
        local data, eof = p:read({
            stream = 'stdout',
            max_bytes = 4096,
            timeout_ms = remaining,
        })
        if data and #data > 0 then
            acc = acc .. data
            if acc:find(needle, 1, true) then
                return acc
            end
        end
        if eof then
            return acc
        end
    end
    fail(('timed out waiting for %q; got %q'):format(needle, acc))
end

read_until('ready', 300000)

p:write('BEGIN\n', { timeout_ms = 10000 })
read_until('begin', 300000)

-- The inter-message wait happens here: the host only writes the next command
-- after observing the previous response, exactly like a real client.
p:write('ID\n', { timeout_ms = 10000 })
local with_id = read_until('id=', 300000)
if not with_id:find('id=%d+') then
    fail(('unexpected transaction id response: %q'):format(with_id))
end

p:write('COMMIT\n', { timeout_ms = 10000 })
p:close_stdin()
read_until('commit', 300000)

p:join({ timeout_ms = 300000 })

print('ok')
io.stdout:flush()
os.exit(0)
