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
    cg.first = wasm.bind(cg.first_module)
    cg.second = wasm.bind(cg.second_module)
end)

g.after_each(function(cg)
    pcall(cg.first.release, 'http-handler')
    pcall(cg.second.release, 'http-handler')
    wasm.unregister_callback('http-handler')
end)

g.after_all(function(cg)
    wasm.drop(cg.first_module)
    wasm.drop(cg.second_module)
end)

g.test_guest_calls_lua_http_handler = function(cg)
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
