local fio = require('fio')

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

return utils