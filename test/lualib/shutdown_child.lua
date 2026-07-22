local fio = require('fio')

local workdir = fio.tempdir()
box.cfg{
    memtx_dir = workdir,
    wal_dir = workdir,
    vinyl_dir = workdir,
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
local run_component = fio.pathjoin(
    root,
    'test/lualib/wasm_component_test/cancel_run_test/adder.component.wasm'
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
_G.shutdown_session = wasm.open_session(session_module)
report_setup('session started')

local run_module = wasm.load(run_component, {
    env = {INF = 'True'},
    stdout = fio.pathjoin(workdir, 'run.log'),
})
_G.shutdown_run = wasm.run(run_module)
report_setup('background run started')

_G.shutdown_interpreter = wasm.load(idle_component, {
    interpreter = true,
})
report_setup('interpreter started')

io.stdout:write('ready\n')
io.stdout:flush()
assert(io.stdin:read('*l') == 'shutdown')

os.exit(0)
