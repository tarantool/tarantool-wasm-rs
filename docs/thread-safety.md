# Tarantool thread-safety boundary

Tarantool's database, transaction, session, fiber-local diagnostics, tuple,
iterator, key-def, and tuple-format APIs are treated as TX-thread-only. Host
bindings dispatch these operations through `Executor::execute` and call
`debug_assert_tx_thread()` immediately before entering Tarantool. The same
assertion protects TX-owned iterator destruction.

`RuntimeState` records the thread that initializes the Lua module as TX. The
module must therefore be initialized by Tarantool on its Lua/TX thread. Debug
and CI builds turn a violation into a panic at the offending API boundary.

## TX-thread-only operations

| Area | Operations |
| --- | --- |
| Database and schema | space/index lookup, schema version, index reads and writes, sequences |
| Transactions | transaction state, begin, commit, rollback, isolation, synchronous commit flag |
| Session | current session, iproto send, Lua broadcast |
| Diagnostics | read, set, and clear Tarantool's fiber-local last error |
| Tuples | creation, update/upsert, reference counts, fields, validation, comparison, iterators |
| Metadata | key-def and tuple-format creation, use, reference counts, and destruction; APIs returning box-region memory are enclosed in a savepoint guard that truncates the current TX fiber region after copying |
| Runtime integration | `coio_call`, shutdown trigger callbacks, and fiber operations |
| Metrics | `tarantool_uptime` |

During `wasm.run`, transaction begin reschedules the Wasmtime task onto TX and
switches its executor to direct execution until commit or rollback. Calls made
through `wasm.call`, sessions, or the interpreter cannot preserve a Tarantool
fiber transaction across host calls and reject transaction begin.

Index and tuple iterators are `Send` only as opaque transport objects. Their
pointers are never dereferenced on a worker thread, host resource calls are
serialized, and their finalizers run on TX. If a guest does not explicitly
drop a resource, destruction of the Wasmtime resource table queues its
non-yielding finalizer back to TX. This is the safety condition for the narrow
`unsafe impl Send` declarations.

## Operations allowed outside TX

| Area | MT-safety and lifetime guarantee |
| --- | --- |
| `tnt_tx_push` / `tnt_tx_flush` | Tarantool's external-thread bridge is specifically required to be called outside TX. While the runtime is `Running`, a queued `Box<TxCtx>` remains allocated until the TX callback takes ownership. Submission is serialized with the transition to `Stopping`, so no worker can call these functions after shutdown begins. Callbacks submitted by an already active worker during `Stopping` go to an in-process queue drained on TX. The callback asserts that it entered TX. |
| Logging (`say`) | Tarantool creates an on-demand thread-local cord for a native thread, formats through a thread-local buffer, protects logger configuration with a mutex, and emits through the logger file descriptor. The binding passes owned Rust strings for the duration of each call and retains no borrowed pointer afterward. Logger reconfiguration and shutdown remain TX-owned lifecycle operations. |
| MessagePack conversion | Pure Rust decoding/encoding over owned input and output buffers; it does not enter Tarantool or retain pointers into guest memory. |
| zlib | Pure compression/decompression over owned input and output buffers; it does not enter Tarantool or retain pointers into guest memory. |
| Wasmtime/WASI execution | Engine and component objects use their documented thread-safe ownership. A store and instance remain owned by one worker and are not concurrently accessed. |

## Shutdown

The runtime has a process-wide `Running` → `Stopping` → `Stopped` lifecycle.
The Tarantool shutdown trigger first rejects new loads, calls, sessions, and
workers, then increments both Wasmtime engine epochs to interrupt running
stores. It closes sessions and shared interpreters, cancels background runs,
and continues draining callbacks from active workers on TX. Only after the
active worker count reaches zero does it join every worker thread, clear the
module and component registries, and enter `Stopped`.

This ordering is required by the `tnt_tx_push` contract: all external worker
threads are gone before Tarantool proceeds with shutdown.

No other Tarantool C API may be added to a worker-side binding without either
dispatching it to TX with `debug_assert_tx_thread()` or documenting its native
MT-safety and complete ownership/lifetime contract here.
