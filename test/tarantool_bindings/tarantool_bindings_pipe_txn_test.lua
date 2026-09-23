local fio = require('fio')
local popen = require('popen')
local t = require('luatest')

package.path = './test/?.lua;' .. package.path
local utils = require('utils')
local cases = t.helpers.matrix({
    engine = {'memtx', 'vinyl'},
    scenario = {'semantics', 'pool', 'shutdown', 'run'},
})
table.insert(cases, {engine = 'memtx', scenario = 'non_mvcc'})
for _, engine in ipairs({'memtx', 'vinyl'}) do
    table.insert(cases, {engine = engine, scenario = 'cancel_isolated'})
end
for _, scenario in ipairs({'cancel_streams', 'cancel_race', 'shutdown_cpu'}) do
    table.insert(cases, {engine = 'memtx', scenario = scenario})
end
local g = t.group('bindings_pipe_txn_test', cases)

g.after_each(function(cg)
    if cg.child then cg.child:close() end
    if cg.workdir then fio.rmtree(cg.workdir) end
end)

g.test_txn_survives_pipe_round_trips = function(cg)
    local root = fio.abspath(fio.pathjoin(utils.get_test_dir(), '..'))
    local child_script = fio.pathjoin(root,
        'test/tarantool_bindings/wasm_component_test/bindings_pipe_txn_test/child.lua')
    cg.workdir = fio.tempdir()
    local env = os.environ()
    env.WASM_REPO = root
    env.WASM_TXN_WORKDIR = cg.workdir
    env.WASM_TXN_SCENARIO = cg.params.scenario
    env.WASM_TXN_ENGINE = cg.params.engine
    env.LUA_PATH = package.path
    env.LUA_CPATH = package.cpath
    local child, err = popen.new({arg[-1], child_script}, {
        env = env,
        stdout = popen.opts.PIPE,
        stderr = popen.opts.PIPE,
    })
    t.assert(child, err)
    cg.child = child

    -- Allow component compilation, but bound even a complete TX deadlock.
    local out, read_err = child:read({timeout = 180})
    local status, wait_err = child:wait({timeout = 10})
    local stderr = child:read({stderr = true, timeout = 0}) or ''
    local log = assert(io.open(fio.pathjoin(cg.workdir, 'tarantool.log'), 'r'))
    local child_log = log:read('*a')
    log:close()
    t.assert_equals(out, 'ok\n', tostring(read_err) .. '; ' .. stderr .. '; ' .. child_log)
    t.assert(status, tostring(wait_err) .. '; ' .. stderr)
    t.assert_equals(status.state, 'exited')
    t.assert_equals(status.exit_code, 0, stderr)
end
