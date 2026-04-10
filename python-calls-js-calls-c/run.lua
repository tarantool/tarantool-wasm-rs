local fio = require('fio')
local wasm = require('wasm')

box.cfg({
    log_level = 6,
})

local c_uid = wasm.load('./provider-c/cprovider.component.wasm')

local js_uid = wasm.load('./provider-js/jsprovider.wasm', {
    links = {
        ["demo:cross-call/coremath@0.1.0"] = c_uid,
    },
})

local py_uid = wasm.load('./consumer-python/consumer.wasm', {
    links = {
        ["demo:cross-call/math@0.1.0"] = js_uid,
    },
})

local total = wasm.call(py_uid, 'call-chain-add', 20, 22)
local label = wasm.call(py_uid, 'call-chain-label', 20, 22)

print('call-chain-add =>', total)
print('call-chain-label =>', label)
