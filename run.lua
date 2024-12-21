local wasm = require('my_module')
local module = wasm.load("../wasm-component-test/guest-js/add.wasm")
print(module)
wasm.run(module)
