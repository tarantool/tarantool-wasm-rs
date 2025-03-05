local wasm = require('my_module')
local log = require('log')

-- Создаем пространство, если оно не существует
local function ensure_space_exists()
    local space_name = "test_space"

    -- Проверяем, есть ли уже пространство
    if box.space[space_name] == nil then
        log.info("Creating space: %s", space_name)
        box.schema.space.create(space_name, { if_not_exists = true })
        box.space[space_name]:create_index('primary', {
            type = 'tree',
            parts = {1, 'unsigned'},
            if_not_exists = true
        })
        log.info("Space '%s' successfully created", space_name)
    else
        log.info("Space '%s' already exists", space_name)
    end
end

-- Инициализируем Tarantool и создаем пространство перед запуском WASM
box.cfg{}

ensure_space_exists()

print("Initialized")
local module = wasm.load("../wasm-component-test/guest-js/add.wasm")
print("Loaded")
print(module)
print("Running")
local handle = wasm.run(module)
wasm.join(handle)
print("Finished")
