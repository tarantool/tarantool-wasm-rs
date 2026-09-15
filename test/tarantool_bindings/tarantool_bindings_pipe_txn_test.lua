local fio = require('fio')
local popen = require('popen')
local t = require('luatest')

package.path = './test/?.lua;' .. package.path
local utils = require('utils')

local test_group = 'bindings_pipe_txn_test'

local g = t.group(test_group)

g.after_each(function(cg)
    if cg.child ~= nil then
        cg.child:close()
        cg.child = nil
    end
end)

-- Regression test for the deadlock fixed by the dedicated per-transaction
-- Tarantool fiber (see docs/thread-safety.md and the runtime's `tx_fiber`
-- module): a transaction held open across separate pipe messages must not park
-- the TX thread.
--
-- The scenario runs in a child process because, on the buggy runtime, the guest
-- parks TX while waiting for the next message, so no in-process Lua timeout can
-- ever fire. The parent enforces the wall-clock bound and fails fast.
g.test_txn_survives_pipe_round_trips = function(cg)
    local root = fio.abspath(fio.pathjoin(utils.get_test_dir(), '..'))
    local child_script = fio.pathjoin(
        utils.get_test_dir(),
        'tarantool_bindings',
        'wasm_component_test',
        'bindings_pipe_txn_test',
        'child.lua'
    )

    local env = os.environ()
    env.WASM_REPO = root
    env.LUA_PATH = package.path
    env.LUA_CPATH = package.cpath

    local child, err = popen.new({ arg[-1], child_script }, {
        env = env,
        stdin = popen.opts.PIPE,
        stdout = popen.opts.PIPE,
        stderr = popen.opts.PIPE,
    })
    t.assert(child, err)
    cg.child = child

    local out, read_err = child:read({ timeout = 900 })
    if out == nil then
        local stderr = child:read({ stderr = true, timeout = 0 }) or ''
        t.fail(('child did not finish (transaction deadlock?): %s; stderr: %s'):format(
            read_err, stderr
        ))
    end
    t.assert_equals(out, 'ok\n')

    local status, wait_err = child:wait({ timeout = 30 })
    if status == nil then
        local stderr = child:read({ stderr = true, timeout = 0 }) or ''
        t.fail(('child did not exit: %s; stderr: %s'):format(wait_err, stderr))
    end
    t.assert_equals(status.state, 'exited')
    t.assert_equals(status.exit_code, 0)
end
