local fio = require('fio')
local popen = require('popen')
local t = require('luatest')

package.path = './test/?.lua;' .. package.path
local utils = require('utils')

local g = t.group('shutdown_test')

g.after_each(function(cg)
    if cg.child ~= nil then
        cg.child:close()
    end
end)

g.test_workers_are_joined_before_shutdown = function(cg)
    local root = fio.abspath(fio.pathjoin(utils.get_test_dir(), '..'))
    local child_script = fio.pathjoin(
        utils.get_test_dir(), 'lualib', 'shutdown_child.lua'
    )
    local env = os.environ()
    env.WASM_REPO = root
    env.LUA_PATH = package.path
    env.LUA_CPATH = package.cpath

    local child, err = popen.new({arg[-1], child_script}, {
        env = env,
        stdin = popen.opts.PIPE,
        stdout = popen.opts.PIPE,
        stderr = popen.opts.PIPE,
    })
    t.assert(child, err)
    cg.child = child

    local ready, ready_err = child:read({timeout = 120})
    if ready == nil then
        local stderr = child:read({stderr = true, timeout = 0}) or ''
        t.fail(('child setup failed: %s; stderr: %s'):format(
            ready_err, stderr
        ))
    end
    t.assert_equals(ready, 'ready\n')

    local written, write_err = child:write('shutdown\n', {timeout = 5})
    t.assert(written, write_err)

    local status, wait_err = child:wait({timeout = 30})
    if status == nil then
        local stderr = child:read({stderr = true, timeout = 0}) or ''
        t.fail(('%s; stderr: %s'):format(wait_err, stderr))
    end
    t.assert_equals(status.state, 'exited')
    if status.exit_code ~= 0 then
        local stderr = child:read({stderr = true, timeout = 0}) or ''
        t.fail(('child exited with %d; stderr: %s'):format(
            status.exit_code, stderr
        ))
    end
end
