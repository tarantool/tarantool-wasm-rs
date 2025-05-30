local fiber = require('fiber')
local wasm  = require('my_module')

box.cfg{
    log_level = 6,
}

local module = wasm.load(...)
local handle = wasm.run(module)

local f = fiber.create(function()
    print(fiber:self():id())
    box.begin()
    local counter = 0
    while true do
        for i = 1, 1e7 do
            counter = counter + 1
        end
        fiber.sleep(0.5)
    end
    box.commit()
end)
f:name("TX_LOOP")

print("1. !!!")
f:set_joinable(true)
f:join()
print("2. !!!")

wasm.join(handle)
