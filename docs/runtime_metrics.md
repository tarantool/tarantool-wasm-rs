# Runtime metrics

`wasm.stats()` returns a process-wide snapshot. Calling it does not pause
workers, instantiate a component, or reset counters.

## Reading a snapshot

The API returns ordinary Lua tables, so applications can periodically export
the values to their existing monitoring system.

```lua
local wasm = require('wasm')

local before = wasm.stats()
local result = wasm.call(module, 'example:api/run', request)
local after = wasm.stats()

local calls = after.guest_execution.count -
              before.guest_execution.count
local elapsed_ns = after.guest_execution.total_ns -
                   before.guest_execution.total_ns

if calls > 0 then
    print(('average guest time: %.0f ns'):format(elapsed_ns / calls))
end
```

All counters and latency values are cumulative from the time the native
`wasm` module is loaded. Durations use nanoseconds. A duration group has
`count`, `total_ns`, and `max_ns`; the average is derived as
`total_ns / count`. The snapshot operation reads atomics independently, so a
concurrent call may finish between two fields being read. Treat one snapshot
as observational data rather than a transactional view.

## Snapshot schema

```lua
{
    compilation = {count = 0, total_ns = 0, max_ns = 0},
    worker_start = {count = 0, total_ns = 0, max_ns = 0},
    runtime_setup = {count = 0, total_ns = 0, max_ns = 0},
    store_setup = {count = 0, total_ns = 0, max_ns = 0},
    instantiation = {count = 0, total_ns = 0, max_ns = 0},
    argument_lowering = {count = 0, total_ns = 0, max_ns = 0},
    guest_execution = {count = 0, total_ns = 0, max_ns = 0},
    result_lifting = {count = 0, total_ns = 0, max_ns = 0},
    worker_wait = {count = 0, total_ns = 0, max_ns = 0},
    tx_wait = {count = 0, total_ns = 0, max_ns = 0},
    lua_callback = {count = 0, total_ns = 0, max_ns = 0},

    fuel = {executions = 0, consumed = 0},
    current = {
        modules = 0,
        sessions = 0,
        runs = 0,
        interpreters = 0,
    },
    cache = {
        components = {hits = 0, misses = 0},
        exports = {hits = 0, misses = 0},
    },
    traps = {
        total = 0,
        by_kind = {},
    },
    conversion_errors = {
        total = 0,
        by_kind = {},
    },
    memory = {
        current_bytes = 0,
        peak_bytes = 0,
    },
}
```

`compilation` measures component compilation or trusted precompiled-artifact
deserialization together with `instantiate_pre` preparation. Component cache
hits do not add a compilation sample. Explicit `wasm.precompile` operations
also add a sample.

The call path is split into additional diagnostic spans. `worker_start`
measures the delay from submitting a `call` or `batch` until a stateless pool
worker begins the job, including queueing. `runtime_setup` measures creation of
the current-thread Tokio runtime used once by each stateless pool worker,
session, and shared interpreter. `store_setup` covers creation and
configuration of the Wasmtime store used by those execution modes.

`instantiation` covers creation of a Wasmtime component instance for one-shot
calls, sessions, shared interpreters, and `wasi:cli/run`. With the pooling
allocator enabled, this includes acquiring and initializing an allocator slot;
returning the slot when its store is dropped happens outside this span.
`argument_lowering` measures conversion from the runtime value representation
to component-model values, including resource handling for persistent
instances. `guest_execution` covers only the exported guest function or
`wasi:cli/run` call. `result_lifting` measures conversion back to the runtime
value representation. The direct MessagePack interpreter protocol does not
use the argument-lowering or result-lifting timers.

`worker_wait` measures end-to-end time from dispatch until a worker result is
consumed on the Tarantool thread. It intentionally overlaps queueing,
worker-side setup, instantiation, conversion, and guest-execution spans, so the
duration groups must not be added together. In a stateless batch,
worker-start and wait samples are counted per batch, while store,
instantiation, lowering, guest, and lifting samples are counted per logical
call. Runtime-setup samples are recorded when pool workers are created, not for
each stateless call or batch. A persistent session or interpreter records
runtime/store setup and instantiation once when its worker starts; lowering,
guest, lifting, and wait continue to be recorded for subsequent calls.

`tx_wait` starts immediately before a worker submits work to Tarantool's TX
thread and stops when that closure begins running. Direct execution on the TX
thread does not add a sample. `lua_callback` measures callback registry lookup,
MessagePack/Lua conversion, coroutine resume cycles, and response encoding.

`fuel.executions` counts guest calls for which fuel was enabled and readable
before and after execution. `fuel.consumed` is the sum of the before/after
fuel deltas, including calls that end in a trap.

`current.modules`, `current.sessions`, `current.runs`, and
`current.interpreters` are gauges read from the runtime registries. A finished
background run remains counted until its join handle is consumed.
`current.interpreters` counts shared interpreter instances, not module handles
that point to the same interpreter.

Component cache counters describe prepared-component lookups. Export cache
counters describe WIT export-index resolution. A source change at the same
path is a component cache miss.

Trap kinds include `out_of_fuel`, `interrupt`, `memory_out_of_bounds`,
`stack_overflow`, `unreachable`, `numeric`, `invalid_reference`,
`component_reentrancy`, `async_deadlock`, `host`, and `other`. Only observed
guest execution failures are classified. Conversion-error kinds identify the
boundary that rejected a value: `arguments`, `results`,
`resource_arguments`, `resource_results`, `callback_request`, or
`callback_response`. A kind is absent from `by_kind` until its first error.

`memory.current_bytes` is the sum of allowed linear-memory allocations owned
by active Wasmtime stores. Failed growth attempts are rolled back.
`memory.peak_bytes` is the largest observed process-wide current value and
does not decrease when a store is dropped. These values cover WebAssembly
linear memory, not native compiler code, runtime bookkeeping, tables, or the
Tarantool process RSS. They also exclude virtual-address reservations and any
resident pages retained by idle Wasmtime allocator slots.
