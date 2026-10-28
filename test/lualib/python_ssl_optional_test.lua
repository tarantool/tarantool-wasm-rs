local ffi = require('ffi')
local fio = require('fio')
local popen = require('popen')
local t = require('luatest')

local g = t.group('python_ssl_optional', {
    {library = 'missing'},
    {library = 'incompatible'},
})

g.after_each(function(cg)
    if cg.child ~= nil then
        cg.child:close()
    end
end)

g.test_native_module_has_no_openssl_dependency = function(cg)
    local module_path = fio.abspath(fio.pathjoin(
        os.getenv('TWASM_MODULE_DIR') or '.rocks/lib/tarantool',
        ffi.os == 'OSX' and 'wasm.dylib' or 'wasm.so'
    ))
    local dependencies = ffi.os == 'OSX'
        and {'/usr/bin/otool', '-L', module_path}
        or {'/usr/bin/readelf', '-d', module_path}
    local undefined = ffi.os == 'OSX'
        and {'/usr/bin/nm', '-u', module_path}
        or {'/usr/bin/nm', '-D', '-u', module_path}
    local commands = {dependencies, undefined}
    local prefixes = {
        'ACCESS_DESCRIPTION_', 'ASN1_', 'BIO_', 'BN_', 'COMP_', 'CRYPTO_',
        'DH_', 'DIST_POINT_', 'EC_', 'ERR_', 'EVP_', 'GENERAL_', 'OBJ_',
        'OCSP_', 'OPENSSL_', 'OpenSSL_', 'PEM_', 'PKCS', 'RAND_', 'RSA_',
        'SSL_', 'TLS_', 'X509_', 'X509V3_', 'd2i_', 'i2d_',
    }

    for _, command in ipairs(commands) do
        cg.child = assert(popen.new(command, {
            env = os.environ(),
            stdout = popen.opts.PIPE,
        }))

        local output = cg.child:read({timeout = 30})
        local status = cg.child:wait({timeout = 30})
        t.assert_equals(status.state, 'exited')
        t.assert_equals(status.exit_code, 0, output)
        t.assert_not_str_contains(output, 'libssl')
        t.assert_not_str_contains(output, 'libcrypto')
        for symbol in output:gmatch('[%w_]+') do
            symbol = symbol:gsub('^_', '')
            for _, prefix in ipairs(prefixes) do
                t.assert_not_equals(symbol:sub(1, #prefix), prefix, symbol)
            end
        end

        cg.child:close()
        cg.child = nil
    end
end

g.test_python_survives_unavailable_ssl = function(cg)
    local env = os.environ()
    env.LUA_PATH = package.path
    env.LUA_CPATH = package.cpath
    env.WASM_PYTHON = fio.abspath('guests/python/interpreter/adder.wasm')
    env.TWR_OPENSSL_LIBRARY = '/nonexistent/twr-openssl/libssl.so.3'
    if cg.params.library == 'incompatible' then
        env.TWR_OPENSSL_LIBRARY = ffi.os == 'OSX'
            and '/usr/lib/libSystem.B.dylib' or 'libc.so.6'
    end

    -- A fresh process also excludes a previously initialized SSL backend.
    local script = [[
        local wasm = require('wasm')
        local module = wasm.load(os.getenv('WASM_PYTHON'), {
            interpreter = true,
        })
        assert(wasm.interpreter_exec(module, [=[
import ssl
from wit_world.imports import ssl_bio

def answer():
    return 42

def ssl_failures():
    for operation in (
        lambda: ssl.RAND_bytes(8),
        ssl.RAND_status,
        lambda: ssl.create_default_context(),
        lambda: ssl.OPENSSL_VERSION,
    ):
        try:
            operation()
        except ssl.SSLError as error:
            assert 'Python SSL unavailable:' in str(error), str(error)
        else:
            raise AssertionError('SSL operation unexpectedly succeeded')
    return True

def raw_ssl_failure():
    try:
        ssl_bio.random_bytes(8)
    except Exception as error:
        return 'Python SSL unavailable:' in error.value.message
    raise AssertionError('Raw SSL operation unexpectedly succeeded')
]=]))
        assert(wasm.interpreter_call(module, 'answer') == 42)
        for _ = 1, 2 do
            assert(wasm.interpreter_call(module, 'ssl_failures'))
            assert(wasm.interpreter_call(module, 'raw_ssl_failure'))
            assert(wasm.interpreter_call(module, 'answer') == 42)
        end
        assert(wasm.drop(module))
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
end
