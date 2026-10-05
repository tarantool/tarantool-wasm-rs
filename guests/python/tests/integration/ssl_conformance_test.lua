local t = require('luatest')
local wasm = require('wasm')
local fio = require('fio')
local json = require('json')
local g = t.group('ssl_conformance')

g.before_each(function(cg) cg.guest_tmp = fio.tempdir() end)
g.after_each(function(cg) if cg.guest_tmp then fio.rmtree(cg.guest_tmp) end end)

g.test_upstream_portable_cases = function(cg)
    local module = wasm.load(fio.abspath(
        'guests/python/tests/components/ssl_conformance/conformance.wasm'), {
        inherit_network = true,
        allowed_ips = {'127.0.0.1', '::1'},
        preopened_dirs = {{fio.abspath(
            'guests/python/tests/conformance/upstream/cpython-3.12.11'),
            '/ssl-conformance', perms = 'ro'},
            {cg.guest_tmp, '/tmp', perms = 'rw'}},
    })
    local ok, result = pcall(wasm.call, module, 'run')
    wasm.drop(module)
    t.assert(ok, tostring(result))
    local report = json.decode(result)
    local report_path = os.getenv('TWR_SSL_CONFORMANCE_REPORT')
    if report_path then
        local file = assert(io.open(report_path, 'wb'))
        file:write(result)
        file:close()
    end
    t.assert_equals(report.cpython_version, '3.12.11')
    t.assert_equals(report.source_sha256,
        '9163dc7b323485573b2325f5c5a57afdc2786ef2c4466a118fadd626a320bb07')
    t.assert_equals(report.ordinary_unittest_failure_recorded, true)
    t.assert_gt(#report.selected, 0)
    t.assert_equals(report.tests_run, #report.selected)
    t.assert_equals(report.failures, {})
    t.assert_equals(report.errors, {})
    t.assert_equals(report.skipped, {})
    t.assert_equals(report.expected_failures, {})
    t.assert_equals(report.unexpected_successes, {})
    t.assert_equals(#report.passed, #report.selected)
end
