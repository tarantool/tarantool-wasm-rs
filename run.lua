local wasm = require('my_module')
local module = wasm.load("/home/maxim/dev/tarantool-wasm/samples/00-basic/hello-qjs/index.wasm")
print(module)
wasm.run(module)
