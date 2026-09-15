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
| Lua callbacks | registry lookup, MessagePack-to-Lua conversion, closure invocation, and result conversion |
| Diagnostics | read, set, and clear Tarantool's fiber-local last error |
| Tuples | creation, update/upsert, reference counts, fields, validation, comparison, iterators |
| Metadata | key-def and tuple-format creation, use, reference counts, and destruction; APIs returning box-region memory are enclosed in a savepoint guard that truncates the current TX fiber region after copying |
| Runtime integration | `coio_call`, `coio_wait`, shutdown trigger callbacks, and fiber operations |
| Metrics | `tarantool_uptime` |

During `wasm.run` or `wasm.pipe`, transaction begin spawns a dedicated
Tarantool fiber that owns the transaction. While it is open, every host call
dispatched through `Executor::TxFiber` runs on that same fiber (so the
fiber-local transaction is preserved), and the fiber waits for the next call
on a nonblocking Unix socket through `coio_wait`. This yields to the TX
event loop without occupying a worker in Tarantool's shared eio pool. The
guest itself stays on its own OS thread, so blocking on WASI
I/O between statements (for example waiting for the next client message on a
pipe) cannot stall Tarantool's scheduler. Calls made through `wasm.call`,
sessions, or the interpreter cannot preserve a Tarantool fiber transaction
across host calls and reject transaction begin.

Transactions now yield between host calls. **Memtx transactions accessing data
require `memtx_use_mvcc_engine = true`** to survive these yields; with MVCC
disabled, Tarantool aborts them on yield and commit returns an error. Earlier
non-yielding `wasm.run` transactions could work with MVCC disabled. This is a
compatibility limitation of keeping guest execution off TX. Vinyl uses its
normal cooperative transaction semantics.

A failed duplicate begin preserves the existing transaction. Commit and
rollback release its owner even on failure; guest exit, trap, or cancellation
closes the queue and wakes the owner to roll back any unfinished transaction.

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
| Wasmtime/WASI execution | Engine and component objects use their documented thread-safe ownership. Each engine owns a synchronized pooling allocator; allocator slots return to that same engine when a store is dropped. Stateless workers reuse their OS thread and Tokio runtime, but each store and instance remains owned by one worker job and is not concurrently accessed. |

## Shutdown

The runtime has a process-wide `Running` → `Stopping` → `Stopped` lifecycle.
The Tarantool shutdown trigger first rejects new loads, calls, sessions, and
workers, then increments both Wasmtime engine epochs to interrupt running
stores. It closes the stateless pool's command senders, sessions, and shared
interpreters, cancels background runs, and continues draining callbacks from
active workers on TX. Pool workers finish cleanup for their current job and
then exit. Jobs still waiting in bounded queues are rejected so they cannot
create a new store after the shutdown epoch increment. Only after the active
worker count reaches zero does the runtime join every worker thread, clear the
module and component registries, and enter `Stopped`.

This ordering is required by the `tnt_tx_push` contract: all external worker
threads are gone before Tarantool proceeds with shutdown.

No other Tarantool C API may be added to a worker-side binding without either
dispatching it to TX with `debug_assert_tx_thread()` or documenting its native
MT-safety and complete ownership/lifetime contract here.
