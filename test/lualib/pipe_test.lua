local t = require('luatest')
local wasm = require('wasm')
local fiber = require('fiber')

local g = t.group('pipe')
local wasm_path = 'test/lualib/wasm_component_test/cancel_run_test/adder.component.wasm'

g.before_each(function(cg)
    cg.module = wasm.load(wasm_path, {
        inherit_env = false,
        env = {PIPE_ECHO = '1'},
        inherit_stdin = false,
        inherit_stdout = false,
        inherit_stderr = false,
    })
end)

g.after_each(function(cg)
    if cg.process ~= nil then
        pcall(cg.process.close, cg.process, {timeout_ms = 5000})
    end
    if cg.module ~= nil then
        pcall(wasm.drop, cg.module)
    end
end)

g.test_binary_round_trip_and_eof = function(cg)
    cg.process = wasm.pipe.spawn(cg.module, nil, {capacity = 64})
    local payload = 'left\0right\255'
    t.assert(cg.process:write(payload, {timeout_ms = 1000}))
    t.assert(cg.process:close_stdin())
    local data, eof = cg.process:read({max_bytes = 64, timeout_ms = 1000})
    t.assert_equals(data, payload)
    t.assert_not(eof)
    data, eof = cg.process:read({max_bytes = 64, timeout_ms = 1000})
    t.assert_equals(data, '')
    t.assert(eof)
    t.assert(cg.process:join({timeout_ms = 5000}))
end

g.test_atomic_write_timeout = function(cg)
    wasm.drop(cg.module)
    cg.module = wasm.load(wasm_path, {
        inherit_env = false,
        env = {PIPE_ECHO = '1', PIPE_DELAY = '1'},
        inherit_stdin = false,
        inherit_stdout = false,
        inherit_stderr = false,
    })
    cg.process = wasm.pipe.spawn(cg.module, nil, {capacity = 4})
    t.assert(cg.process:write('abcd', {timeout_ms = 1000}))
    local ok, err = pcall(cg.process.write, cg.process, 'ef', {timeout_ms = 20})
    t.assert_not(ok)
    t.assert_str_contains(tostring(err), 'timed out')
    t.assert(cg.process:close_stdin())
    local data = cg.process:read({max_bytes = 8, timeout_ms = 1000})
    t.assert_equals(data, 'abcd')
end

g.test_read_any_drains_both_outputs = function(cg)
    wasm.drop(cg.module)
    cg.module = wasm.load(wasm_path, {
        inherit_env = false,
        env = {PIPE_ECHO = '1', PIPE_STDERR = '1'},
        inherit_stdin = false,
        inherit_stdout = false,
        inherit_stderr = false,
    })
    cg.process = wasm.pipe.spawn(cg.module, nil, {capacity = 16})
    t.assert(cg.process:write('both', {timeout_ms = 1000}))
    t.assert(cg.process:close_stdin())
    local values = {stdout = '', stderr = ''}
    while true do
        local stream, data, eof = cg.process:read_any({max_bytes = 16, timeout_ms = 1000})
        if stream == nil then
            t.assert(eof)
            break
        end
        values[stream] = values[stream] .. data
    end
    t.assert_equals(values, {stdout = 'both', stderr = 'both'})
end

g.test_static_stdio_override_is_rejected = function(cg)
    local ok, err = pcall(wasm.pipe.spawn, cg.module, {inherit_stdout = false})
    t.assert_not(ok)
    t.assert_str_contains(tostring(err), 'must not set `inherit_stdout`')
end

g.test_join_timeout_then_cancel_is_cached = function(cg)
    wasm.drop(cg.module)
    cg.module = wasm.load(wasm_path, {
        inherit_env = false,
        env = {PIPE_ECHO = '1', PIPE_DELAY = '1'},
        inherit_stdin = false,
        inherit_stdout = false,
        inherit_stderr = false,
    })
    cg.process = wasm.pipe.spawn(cg.module, nil, {capacity = 64})
    local ok, err = pcall(cg.process.join, cg.process, {timeout_ms = 20})
    t.assert_not(ok)
    t.assert_str_contains(tostring(err), 'timed out')
    t.assert(cg.process:is_alive())
    t.assert(cg.process:cancel())
    ok, err = pcall(cg.process.join, cg.process, {timeout_ms = 5000})
    t.assert_not(ok)
    t.assert_str_contains(tostring(err), 'canceled')
    local again_ok, again_err = pcall(cg.process.join, cg.process, {timeout_ms = 100})
    t.assert_not(again_ok)
    t.assert_str_contains(tostring(again_err), 'canceled')
end

g.test_module_drop_does_not_stop_pipe = function(cg)
    cg.process = wasm.pipe.spawn(cg.module, nil, {capacity = 64})
    t.assert(wasm.drop(cg.module))
    cg.module = nil
    t.assert(cg.process:write('alive', {timeout_ms = 1000}))
    t.assert(cg.process:close_stdin())
    local data = cg.process:read({max_bytes = 64, timeout_ms = 1000})
    t.assert_equals(data, 'alive')
    t.assert(cg.process:join({timeout_ms = 5000}))
end

g.test_close_is_idempotent = function(cg)
    cg.process = wasm.pipe.spawn(cg.module, nil, {capacity = 64})
    t.assert(cg.process:close({timeout_ms = 5000}))
    t.assert(cg.process:close({timeout_ms = 5000}))
    t.assert_not(cg.process:is_alive())
end

g.test_is_alive_tracks_worker_before_join = function(cg)
    cg.process = wasm.pipe.spawn(cg.module, nil, {capacity = 64})
    t.assert(cg.process:close_stdin())
    while true do
        local stream = cg.process:read_any({max_bytes = 64, timeout_ms = 1000})
        if stream == nil then break end
    end
    local deadline = fiber.time() + 2
    while cg.process:is_alive() and fiber.time() < deadline do
        fiber.sleep(0.001)
    end
    t.assert_not(cg.process:is_alive())
    t.assert(cg.process:join({timeout_ms = 1000}))
end
