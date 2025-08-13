local t = require('luatest')
local luawasm = require('luawasm')
local fio = require('fio')
local json = require('json')

package.path = "./test/?.lua;" .. package.path
local utils = require('utils')

local test_group = "bindings_zlib_test"

local g = t.group(test_group)

local path = fio.pathjoin(utils.get_test_dir(), "bindings", "wasm_component_test",  test_group, "python")
local m = luawasm:new({ dir = path })

local function to_bytes(str)
    return { string.byte(str, 1, #str) }
end

local function from_bytes(bytes)
    return string.char(unpack(bytes))
end

local function copy_bytes(arr)
    local out = {}
    for i = 1, #arr do out[i] = arr[i] end
    return out
end

g.test_compress_decompress = function(cg)
    local input = "The quick brown fox jumps over the lazy dog"
    local input_bytes = to_bytes(input)

    local compressed = m:t_compress(input_bytes, 6)
    t.assert_type(compressed, "table")
    t.assert_gt(#compressed, 0)

    local decompressed = m:t_decompress(compressed)
    local output = from_bytes(decompressed)

    t.assert_equals(output, input)
end

g.test_crc32 = function(cg)
    local input = "123456789"
    local bytes = to_bytes(input)

    local crc = m:t_crc32(bytes)
    t.assert_equals(crc, 0xcbf43926)
end

g.test_adler32 = function(cg)
    local input = "123456789"
    local bytes = to_bytes(input)

    local adler = m:t_adler32(bytes)
    t.assert_equals(adler, 0x091e01de)
end

g.test_decompress_invalid_garbage = function(cg)
    local garbage = to_bytes("\001\002\003\004\005plain-text-not-zlib")
    local out = m:t_decompress(garbage)
    t.assert_type(out, "table")
    t.assert_equals(#out, 0, "decompress(garbage) must fail and return empty result")
end

g.test_decompress_truncated_stream = function(cg)
    local input = to_bytes("some data to compress and then cut")
    local compressed = m:t_compress(input, 6)
    t.assert_gt(#compressed, 0)

    local truncated = {}
    local cut = math.min(5, #compressed)
    for i = 1, cut do truncated[i] = compressed[i] end

    local out = m:t_decompress(truncated)
    t.assert_type(out, "table")
    t.assert_equals(#out, 0, "decompress(truncated) must fail and return empty result")
end

g.test_decompress_corrupted_checksum = function(cg)
    local input = to_bytes("checksum should not match after corruption")
    local compressed = m:t_compress(input, 6)
    t.assert_gt(#compressed, 0)

    local corrupted = copy_bytes(compressed)
    corrupted[#corrupted] = (255 - corrupted[#corrupted]) % 256

    local out = m:t_decompress(corrupted)
    t.assert_type(out, "table")
    t.assert_equals(#out, 0, "decompress(corrupted) must fail and return empty result")
end
