local fio = require('fio')
local popen = require('popen')
local t = require('luatest')

local g = t.group('cache', {
    {home = 'missing'},
    {home = 'file'},
    {home = 'writable'},
})

g.after_each(function(cg)
    if cg.child ~= nil then
        cg.child:close()
    end

    if cg.workdir ~= nil then
        fio.rmtree(cg.workdir)
    end
end)

g.test_load_with_unavailable_or_writable_cache = function(cg)
    cg.workdir = fio.tempdir()
    local blocked = fio.pathjoin(cg.workdir, 'blocked')
    local file = assert(io.open(blocked, 'w'))
    file:close()

    local home = blocked
    if cg.params.home == 'missing' then
        -- A file as parent makes the missing home uncreatable, even as root.
        home = fio.pathjoin(blocked, 'home')
    elseif cg.params.home == 'writable' then
        home = fio.pathjoin(cg.workdir, 'home')
        t.assert(fio.mkdir(home))
    end

    local env = os.environ()
    env.HOME = home
    env.XDG_CACHE_HOME = fio.pathjoin(home, '.cache')
    env.LUA_PATH = package.path
    env.LUA_CPATH = package.cpath
    env.WASM_COMPONENT = fio.abspath(fio.pathjoin(
        'test', 'lualib', 'wasm_component_test', 'export_test', 'c',
        'adder.component.wasm'
    ))
    env.WASM_ARTIFACT = fio.pathjoin(cg.workdir, 'adder.cwasm')

    -- Runtime initialization must happen in a fresh process for each HOME.
    local script = [[
        local wasm = require('wasm')
        local path = assert(os.getenv('WASM_COMPONENT'))
        local artifact = assert(os.getenv('WASM_ARTIFACT'))
        for _, config in ipairs({{}, {max_instructions = 1000000}}) do
            local module = wasm.load(path, config)
            assert(wasm.call(module, 't-s16', 42) == 42)
            assert(wasm.drop(module))

            assert(wasm.precompile(path, artifact, config))
            config.precompiled = true
            module = wasm.load(artifact, config)
            assert(wasm.call(module, 't-s16', 43) == 43)
            assert(wasm.drop(module))
        end
        os.exit(0)
    ]]
    local child, err = popen.new({arg[-1], '-e', script}, {
        env = env,
        stderr = popen.opts.PIPE,
    })
    t.assert(child, err)
    cg.child = child

    local status, wait_err = child:wait({timeout = 120})
    local stderr = child:read({stderr = true, timeout = 0}) or ''
    t.assert(status, ('%s; stderr: %s'):format(wait_err, stderr))
    t.assert_equals(status.state, 'exited', stderr)
    t.assert_equals(status.exit_code, 0, stderr)

    local warning = 'Wasmtime disk cache is unavailable; continuing without it'
    if cg.params.home == 'writable' then
        t.assert_not_str_contains(stderr, warning)
    else
        t.assert_str_contains(stderr, warning)
    end
end
