local wasm = require('wasm')
local log = require('log')

box.cfg{}
box.cfg{log_level=6}

local space_name = 'test_space'
local index_name = 'primary'
local wasm_module_path = ...

box.schema.space.create(space_name, { if_not_exists = true })
log.info("Create space: %s", space_name)
box.space[space_name]:create_index(index_name, {
    type = 'tree',
    parts = {1, 'unsigned'},
    if_not_exists = true
})
log.info("Create index: %s", index_name)

box.schema.sequence.create('id_seq',{min=1000, start=1000})

log.info("Load WASM module...")
local module = wasm.load(wasm_module_path)
log.info("Run WASM module...")
local handle = wasm.run(module)
wasm.join(handle)
log.info("WASM module finished...")
