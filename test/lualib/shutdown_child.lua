local fio = require('fio')
local fiber = require('fiber')

local workdir = fio.tempdir()
box.cfg{
    memtx_dir = workdir,
    wal_dir = workdir,
    vinyl_dir = workdir,
    log = fio.pathjoin(workdir, 'tarantool.log'),
    log_level = 7,
}
box.ctl.set_on_shutdown_timeout(10)

local root = assert(os.getenv('WASM_REPO'))
-- Session and interpreter shutdown only require a live persistent worker.
-- Keep their fixture small so debug Wasmtime compilation does not dominate
-- the shutdown deadline exercised by the parent test.
local idle_component = fio.pathjoin(
    root,
    'test/lualib/wasm_component_test/export_test/c/adder.component.wasm'
)
local tx_component = fio.pathjoin(
    root,
    'test/tarantool_bindings/wasm_component_test/bindings_box_test/c/adder.component.wasm'
)
local run_component = fio.pathjoin(
    root,
    'test/lualib/wasm_component_test/cancel_run_test/adder.component.wasm'
)
local long_call_component = fio.pathjoin(
    root,
    'test/lualib/wasm_component_test/component_limits_test/adder.wasm'
)

local function report_setup(phase)
    io.stderr:write(('shutdown test setup: %s\n'):format(phase))
    io.stderr:flush()
end

local wasm
box.ctl.on_shutdown(function()
    local ok, err = pcall(wasm.load, idle_component)
    assert(not ok)
    assert(tostring(err):match('WASM runtime is stopp'))
end)
wasm = require('wasm')
report_setup('runtime loaded')

local session_module = wasm.load(idle_component)
assert(wasm.call(session_module, 't-s16', 7) == 7)
report_setup('stateless worker pool started')
_G.shutdown_session = wasm.open_session(session_module)
report_setup('session started')

local tx_module = wasm.load(tx_component)
for _ = 1, 64 do
    assert(type(wasm.call(tx_module, 't-schema-version')) == 'number')
end
report_setup('stateless worker pool tx callbacks initialized')
do
    local orphan_binding = wasm.bind(tx_module)
    assert(type(orphan_binding['t-schema-version']()) == 'number')
end
collectgarbage('collect')
report_setup('orphan bound session with tx callback collected')

local run_module = wasm.load(run_component, {
    env = {INF = 'True'},
    stdout = fio.pathjoin(workdir, 'run.log'),
})
_G.shutdown_run = wasm.run(run_module)
report_setup('background run started')

_G.shutdown_pipe = wasm.pipe.spawn(run_module, nil, {capacity = 64})
report_setup('live pipe started')

do
    local orphan = wasm.pipe.spawn(run_module, nil, {capacity = 64})
    assert(orphan:is_alive())
end
collectgarbage('collect')
report_setup('orphan pipe collected')

_G.shutdown_interpreter = wasm.load(idle_component, {
    interpreter = true,
})
report_setup('interpreter started')

_G.shutdown_call_module = wasm.load(long_call_component)
_G.shutdown_calls = {}
local started_calls = 0
for i = 1, 64 do
    _G.shutdown_calls[i] = fiber.create(function()
        started_calls = started_calls + 1
        pcall(wasm.call, _G.shutdown_call_module, 'foo')
    end)
end
while started_calls ~= #_G.shutdown_calls do fiber.yield() end
report_setup('stateless calls running and queued')

io.stdout:write('ready\n')
io.stdout:flush()
assert(io.stdin:read('*l') == 'shutdown')

os.exit(0)
