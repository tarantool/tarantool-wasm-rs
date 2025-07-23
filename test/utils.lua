local fio = require('fio')
local msgpack = require('msgpack')

local utils = {}

utils.get_test_dir = function()
    local info = debug.getinfo(1, 'S')
    local file_path = info.source:sub(2)
    local base_dir = file_path:match("(.*/)")
    return base_dir
end

utils.cleanup_logs = function(path)
    for _, file in ipairs(fio.listdir(path)) do
        if file:match("%.snap$") or file:match("%.xlog$") or file:match("%.vylog$") then
            fio.unlink(fio.pathjoin(path, file))
        end
    end
end

utils.enc = function(val)
    local raw = msgpack.encode(val)
    return { string.byte(raw, 1, #raw) }
end

utils.dec = function(msgpack_bytes)
    local str = string.char(unpack(msgpack_bytes))
    return msgpack.decode(str)
end

utils.dec_ptr = function(ptr)
    local ffi = require('ffi')
    local msgpack = require('msgpack')

    local function safe_decode(p)
        local max = 64
        for i = 1, max do
            local ok, val = pcall(msgpack.decode, ffi.string(p, i))
            if ok then return val end
        end
        return nil, "Failed to decode"
    end

    return safe_decode(ffi.cast("const char*", ptr))
end

utils.read_new_log = function(path)
    path = path or "luatest.log"
    local fh = assert(io.open(path, "r"))
    fh:seek("end")
    local offset = fh:seek()
    fh:close()

    return function()
        local fh = assert(io.open(path, "r"))
        fh:seek("set", offset)
        local content = fh:read("*a")
        fh:close()
        return content
    end
end

return utils