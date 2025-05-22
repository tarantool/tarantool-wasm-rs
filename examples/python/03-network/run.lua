local wasm = require('my_module')
local log = require('log')
local fiber = require("fiber")

local wasm_module_path1 = './server/adder.wasm'
local wasm_module_path2 = './client/adder.wasm'

log.info("Load WASM modules...")
local module1 = wasm.load(wasm_module_path1)
local module2 = wasm.load(wasm_module_path2)

log.info("Run WASM modules...")
local handle1 = wasm.run(module2)

fiber.sleep(3)

local handle2 = wasm.run(module1)

wasm.join(handle1)
wasm.join(handle2)
log.info("WASM module finished...")
