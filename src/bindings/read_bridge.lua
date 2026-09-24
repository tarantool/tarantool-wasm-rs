-- All functions run on Tarantool's TX thread. Rust keeps the read view and
-- iterator closures in the Lua registry; no Lua object crosses to a worker.
local msgpack = require('msgpack')

local function index(view, space_id, index_id)
    local space = view.space[space_id]
    if space == nil then
        error('snapshot does not contain space ' .. space_id)
    end
    local result = space.index[index_id]
    if result == nil then
        error('snapshot does not contain index ' .. index_id)
    end
    return result
end

local function key_from_msgpack(bytes)
    local key = msgpack.decode(bytes)
    if type(key) ~= 'table' and not box.tuple.is(key) then
        error('read key must be a MessagePack array')
    end
    return key
end

return {
    open = function()
        if box.read_view == nil or type(box.read_view.open) ~= 'function' then
            error('snapshot unavailable: Tarantool has no application read-view API')
        end
        local opened, view = pcall(box.read_view.open,
                                   {name = 'wasm experimental read'})
        if not opened then
            error('snapshot unavailable: ' .. tostring(view))
        end
        -- A view can omit whole engines or index kinds. Reject it at open so
        -- the guest never treats a partial database view as a full snapshot.
        local valid, reason = pcall(function()
            for id, space in pairs(box.space) do
                if type(id) == 'number' and id >= 512 then
                    local snapshot_space = view.space[id]
                    if snapshot_space == nil then
                        error('snapshot unavailable: space ' .. space.name ..
                              ' (' .. space.engine .. ') is absent from read view')
                    end
                    for index_id, original_index in pairs(space.index) do
                        if type(index_id) == 'number' and
                           snapshot_space.index[index_id] == nil then
                            error('snapshot unavailable: index ' ..
                                  original_index.name .. ' is absent from read view')
                        end
                    end
                end
            end
        end)
        if not valid then
            view:close()
            error(reason)
        end
        return view
    end,
    close = function(view)
        view:close()
    end,
    space_by_name = function(view, name)
        local space = view.space[name]
        return space and space.id
    end,
    index_by_name = function(view, space_id, name)
        local space = view.space[space_id]
        if space == nil then
            error('snapshot does not contain space ' .. space_id)
        end
        local found = space.index[name]
        return found and found.id
    end,
    get = function(view, space_id, index_id, key)
        local tuple = index(view, space_id, index_id):get(key_from_msgpack(key))
        return tuple and msgpack.encode(tuple)
    end,
    count = function(view, space_id, index_id, mode, key)
        return index(view, space_id, index_id):count(
            key_from_msgpack(key), {iterator = mode})
    end,
    select = function(view, space_id, index_id, mode, key)
        local gen, param, state = index(view, space_id, index_id):pairs(
            key_from_msgpack(key), {iterator = mode})
        return function()
            local next_state, tuple = gen(param, state)
            state = next_state
            -- The Lua-C and LuaJIT-FFI iterator implementations have
            -- different generator result shapes.
            if tuple == nil and box.tuple.is(next_state) then
                tuple = next_state
            end
            return tuple and msgpack.encode(tuple)
        end
    end,
}
