local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local g = t.group('bindings_lua_callback_test')
local path = fio.pathjoin(
    utils.get_test_dir(), 'bindings', 'wasm_component_test',
    'bindings_lua_callback_test', 'python', 'adder.wasm'
)

g.before_all(function(cg)
    cg.first_module = wasm.load(path)
    cg.second_module = wasm.load(path)
    cg.interpreter_module = wasm.load(path, {interpreter = true})
    cg.first = wasm.bind(cg.first_module)
    cg.second = wasm.bind(cg.second_module)
    cg.interpreter = wasm.bind(cg.interpreter_module)
end)

g.after_each(function(cg)
    for _, name in ipairs({
        'http-handler',
        'yield-handler',
        'cycle-handler',
        'coroutine-handler',
        'timeout-handler',
    }) do
        pcall(cg.first.release, name)
        pcall(cg.second.release, name)
        pcall(cg.interpreter.release, name)
        wasm.unregister_callback(name)
    end
end)

g.after_all(function(cg)
    wasm.drop(cg.first_module)
    wasm.drop(cg.second_module)
    wasm.drop(cg.interpreter_module)
end)

g.test_guest_calls_lua_http_handler = function(cg)
    local before = wasm.stats()
    wasm.register_callback('http-handler', function(request)
        return {
            status = 200,
            headers = {['content-type'] = 'text/plain'},
            body = 'hello from Lua: ' .. request.path,
        }
    end)

    local acquired = cg.first.acquire('http-handler')
    t.assert_equals(acquired[1], true)

    local payload = utils.enc({method = 'GET', path = '/health'})
    local result = cg.first.invoke('http-handler', payload)

    t.assert_equals(result[1], true)
    local response = utils.dec(result[2])
    t.assert_equals(response, {
        status = 200,
        headers = {['content-type'] = 'text/plain'},
        body = 'hello from Lua: /health',
    })
    local after = wasm.stats()
    t.assert_ge(after.lua_callback.count, before.lua_callback.count + 1)
    t.assert_ge(after.tx_wait.count, before.tx_wait.count + 1)
end

g.test_callback_replacement = function(cg)
    -- Handler resources may exist before the Lua closure is registered.
    t.assert_equals(cg.first.acquire('http-handler')[1], true)
    t.assert_equals(cg.second.acquire('http-handler')[1], true)

    wasm.register_callback('http-handler', function() return 'first' end)
    local payload = utils.enc({})
    t.assert_equals(utils.dec(cg.first.invoke('http-handler', payload)[2]), 'first')
    t.assert_equals(utils.dec(cg.second.invoke('http-handler', payload)[2]), 'first')

    wasm.register_callback('http-handler', function() return 'second' end)
    t.assert_equals(utils.dec(cg.first.invoke('http-handler', payload)[2]), 'second')
    t.assert_equals(utils.dec(cg.second.invoke('http-handler', payload)[2]), 'second')
end

g.test_unregister_callback = function(cg)
    wasm.register_callback('http-handler', function() return true end)
    t.assert_equals(cg.first.acquire('http-handler')[1], true)
    t.assert(wasm.unregister_callback('http-handler'))
    t.assert_not(wasm.unregister_callback('http-handler'))

    local ok, err = pcall(cg.first.invoke, 'http-handler', utils.enc({}))
    if ok then
        t.assert_equals(err[1], false)
        t.assert_str_contains(err[2], 'is not registered')
    else
        t.assert_str_contains(tostring(err), 'is not registered')
    end
end

g.test_resource_drop_does_not_unregister_callback = function(cg)
    wasm.register_callback('http-handler', function() return 'still-live' end)
    t.assert_equals(cg.first.acquire('http-handler')[1], true)

    cg.first.release('http-handler')
    local result = cg.first.invoke('http-handler', utils.enc({}))
    t.assert_equals(result[1], true)
    t.assert_equals(utils.dec(result[2]), 'still-live')
end

g.test_empty_handler_name_is_rejected = function(cg)
    local result = cg.first.acquire('')
    t.assert_equals(result[1], false)
    t.assert_str_contains(result[2], 'callback name must not be empty')
end

g.test_direct_reentrant_call_is_rejected = function(cg)
    wasm.register_callback('http-handler', function()
        local nested = cg.first.invoke('http-handler', utils.enc({}))
        if not nested[1] then
            error(nested[2])
        end
        return nested
    end)

    local result = cg.first.invoke('http-handler', utils.enc({}))
    t.assert_equals(result[1], false)
    t.assert_str_contains(result[2], 'reentrant WASM call rejected')
end

g.test_cyclic_reentrant_call_is_rejected = function(cg)
    wasm.register_callback('cycle-handler', function(request)
        if request.hop == 1 then
            local nested = cg.second.invoke(
                'cycle-handler', utils.enc({hop = 2})
            )
            if not nested[1] then error(nested[2]) end
            return nested
        end

        local nested = cg.first.invoke('cycle-handler', utils.enc({hop = 3}))
        if not nested[1] then error(nested[2]) end
        return nested
    end)

    local result = cg.first.invoke('cycle-handler', utils.enc({hop = 1}))
    t.assert_equals(result[1], false)
    t.assert_str_contains(result[2], 'reentrant WASM call rejected')
end

g.test_interpreter_reentrant_call_is_rejected = function(cg)
    wasm.register_callback('http-handler', function()
        local nested = cg.interpreter.invoke('http-handler', utils.enc({}))
        if not nested[1] then error(nested[2]) end
        return nested
    end)

    local result = cg.interpreter.invoke('http-handler', utils.enc({}))
    t.assert_equals(result[1], false)
    t.assert_str_contains(result[2], 'reentrant WASM call rejected')
end

g.test_callback_can_yield_tarantool_fiber = function(cg)
    local fiber = require('fiber')
    wasm.register_callback('yield-handler', function(request)
        fiber.yield()
        return request.value + 1
    end)

    local result = cg.first.invoke(
        'yield-handler', utils.enc({value = 41})
    )
    t.assert_equals(result[1], true)
    t.assert_equals(utils.dec(result[2]), 42)
end

g.test_callback_can_sleep_tarantool_fiber = function(cg)
    local fiber = require('fiber')
    wasm.register_callback('yield-handler', function(request)
        fiber.sleep(0.01)
        return request.value + 1
    end)

    local result = cg.first.invoke(
        'yield-handler', utils.enc({value = 41})
    )
    t.assert_equals(result[1], true)
    t.assert_equals(utils.dec(result[2]), 42)
end

g.test_callback_coroutine_is_resumed_until_completion = function(cg)
    wasm.register_callback('coroutine-handler', function(request)
        coroutine.yield('intermediate value is ignored')
        coroutine.yield()
        return request.value + 1
    end)

    local result = cg.first.invoke(
        'coroutine-handler', utils.enc({value = 41})
    )
    t.assert_equals(result[1], true)
    t.assert_equals(utils.dec(result[2]), 42)
end

g.test_callback_timeout_interrupts_lua_code = function(cg)
    wasm.register_callback('timeout-handler', function()
        local value = 0
        while true do value = value + 1 end
    end, {timeout_ms = 10})

    local result = cg.first.invoke('timeout-handler', utils.enc({}))
    t.assert_equals(result[1], false)
    t.assert_str_contains(result[2], 'timed out')
end

g.test_callback_timeout_is_checked_after_fiber_sleep = function(cg)
    local fiber = require('fiber')
    wasm.register_callback('timeout-handler', function()
        fiber.sleep(0.03)
        return true
    end, {timeout_ms = 10})

    local result = cg.first.invoke('timeout-handler', utils.enc({}))
    t.assert_equals(result[1], false)
    t.assert_str_contains(result[2], 'timed out')
end

g.test_callback_timeout_must_be_positive = function()
    local ok, err = pcall(
        wasm.register_callback,
        'timeout-handler',
        function() return true end,
        {timeout_ms = 0}
    )
    t.assert_not(ok)
    t.assert_str_contains(tostring(err), 'must be greater than zero')
end
