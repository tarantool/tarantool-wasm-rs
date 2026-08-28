local fiber = require('fiber')
local fio = require('fio')
local t = require('luatest')
local wasm = require('wasm')

package.path = './test/?.lua;' .. package.path
local utils = require('utils')

local g = t.group('parallel_execution')

local export_component = fio.pathjoin(
    utils.get_test_dir(),
    'lualib', 'wasm_component_test', 'export_test', 'c', 'adder.component.wasm'
)
local run_component = fio.pathjoin(
    utils.get_test_dir(),
    'lualib', 'wasm_component_test', 'cancel_run_test', 'adder.component.wasm'
)

local function line_count(path)
    local file = assert(io.open(path, 'r'))
    local content = file:read('*a')
    file:close()
    local count = 0
    for _ in content:gmatch('[^\n]+') do
        count = count + 1
    end
    return count
end

g.test_concurrent_calls_keep_request_results = function()
    local module_uid = wasm.load(export_component)
    local workers = 32
    local calls_per_worker = 8
    local done = fiber.channel(workers)

    for worker = 1, workers do
        local worker_id = worker
        fiber.create(function()
            local ok, err = pcall(function()
                local export = worker_id % 2 == 0 and 't-s32' or 't-u32'
                for call = 1, calls_per_worker do
                    local expected = worker_id * 1000 + call
                    local actual = wasm.call(module_uid, export, expected)
                    t.assert_equals(tonumber(actual), expected)
                end
            end)
            done:put({ok, err})
        end)
    end

    for _ = 1, workers do
        local result = done:get(30)
        t.assert(result ~= nil, 'parallel call worker timed out')
        t.assert(result[1], tostring(result[2]))
    end
    t.assert(wasm.drop(module_uid))
end

g.test_concurrent_batches_keep_request_results = function()
    local module_uid = wasm.load(export_component)
    local workers = 16
    local batches_per_worker = 4
    local done = fiber.channel(workers)

    for worker = 1, workers do
        local worker_id = worker
        fiber.create(function()
            local ok, err = pcall(function()
                for batch = 1, batches_per_worker do
                    local expected = worker_id * 1000 + batch
                    local actual = wasm.batch(module_uid, {
                        {'t-u32', expected},
                        {'t-s32', expected},
                    })
                    t.assert_equals(tonumber(actual[1]), expected)
                    t.assert_equals(tonumber(actual[2]), expected)
                end
            end)
            done:put({ok, err})
        end)
    end

    for _ = 1, workers do
        local result = done:get(30)
        t.assert(result ~= nil, 'parallel batch worker timed out')
        t.assert(result[1], tostring(result[2]))
    end
    t.assert(wasm.drop(module_uid))
end

g.test_background_runs_make_progress_in_parallel = function()
    local tmpdir = fio.tempdir()
    local outputs = {
        fio.pathjoin(tmpdir, 'first.txt'),
        fio.pathjoin(tmpdir, 'second.txt'),
    }
    local module_uid = wasm.load(run_component)
    local handles = {}

    for index, output in ipairs(outputs) do
        handles[index] = wasm.run(module_uid, {
            stdout = output,
            env = {INF = 'True'},
        })
    end

    local deadline = fiber.clock() + 2
    while (line_count(outputs[1]) < 3 or line_count(outputs[2]) < 3)
        and fiber.clock() < deadline do
        fiber.sleep(0.01)
    end
    t.assert_ge(line_count(outputs[1]), 3)
    t.assert_ge(line_count(outputs[2]), 3)

    t.assert(wasm.cancel(handles[1]))
    t.assert(wasm.cancel(handles[2]))
    t.assert(wasm.drop(module_uid))
end

g.test_concurrent_join_preserves_success_and_trap = function()
    local good = wasm.load(run_component)
    local bad = wasm.load(run_component, {env = {TRAP = 'True'}})
    local good_handle = wasm.run(good)
    local bad_handle = wasm.run(bad)
    local done = fiber.channel(2)

    fiber.create(function()
        done:put({'good', pcall(wasm.join, good_handle)})
    end)
    fiber.create(function()
        done:put({'bad', pcall(wasm.join, bad_handle)})
    end)

    local results = {}
    for _ = 1, 2 do
        local result = done:get(10)
        t.assert(result ~= nil, 'parallel join timed out')
        results[result[1]] = result[2]
    end
    t.assert_equals(results.good, true)
    t.assert_equals(results.bad, false)
    t.assert(wasm.drop(good))
    t.assert(wasm.drop(bad))
end
