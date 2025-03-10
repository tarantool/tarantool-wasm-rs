local wasm = require('my_module')
local log = require('log')

local function ensure_space_exists()
    local space_name = "test_space"

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

local function print_space_contents()
    local space = box.space.test_space
    if space then
        log.info("Dumping contents of 'test_space':")
        for _, tuple in space:pairs() do
            log.info("%s", tuple)
        end
    else
        log.info("Space 'test_space' does not exist")
    end
end

box.cfg{}

ensure_space_exists()

local function on_replace_trigger(old, new, space_name, op)
    local log = require('log')
    
    if op == "INSERT" then
        log.info("TARANTOOL | INSERT into %s: %s", space_name, new)
    elseif op == "DELETE" then
        log.info("TARANTOOL | DELETE from %s: %s", space_name, old)
    elseif op == "UPDATE" then
        log.info("TARANTOOL | UPDATE in %s: old = %s, new = %s", space_name, old, new)
    elseif op == "REPLACE" then
        log.info("TARANTOOL | REPLACE in %s: old = %s, new = %s", space_name, old, new)
    elseif op == "UPSERT" then
        log.info("TARANTOOL | UPSERT in %s: old = %s, new = %s", space_name, old, new)
    else
        log.warn("TARANTOOL | Unknown operation %s in %s", op, space_name)
    end
end

if box.space.test_space then
    box.space.test_space:on_replace(on_replace_trigger)
    log.info("TARANTOOL | on_replace trigger attached to 'test_space'")
end

print("Initialized")
local module = wasm.load("../wasm-component-test/guest-js/add.wasm")
print("Loaded")
print(module)
print("Running")
local handle = wasm.run(module)
wasm.join(handle)
print("Finished")

print_space_contents()
