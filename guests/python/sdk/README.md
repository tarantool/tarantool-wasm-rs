# Tarantool Python API

The `twr-python-sdk` wheel provides `twr_python_sdk.tarantool` for
`tarantool:tarantool@0.2.0` and the runtime's `docs:adder/lua-callback@0.1.3`.
It runs inside a component hosted by this repository's native module.
Importing it does not install standard-library ports or call Tarantool.

## Component setup

Install the wheel and its `msgpack==1.0.3` dependency in the Python paths passed
to componentize-py. The SDK uses MessagePack's pure Python implementation.
Include the database imports in your WIT world:

```wit
include tarantool:tarantool/guest@0.2.0;
```

Fetch that dependency with the repository's Tarantool registry configuration.
You can instead import individual interfaces when the component uses only part
of the API. Compile with the componentize-py instructions in
[the Python README](../README.md).

## Database operations

```python
from twr_python_sdk import tarantool as box

users = box.space["users"]
users.insert((1, "Alice", 10))
users.update(1, [("+", 2, 5)])
user = users.get(1)                       # (1, "Alice", 15), or None
user = users.index["name"].get("Alice")
rows = users.select(limit=100)

with users.iter() as cursor:
    for row in cursor:
        print(row)
```

`Space` supports insert, replace, update, upsert, delete, truncate, get, select
and iter. Key operations use index 0. `Index` also supports min, max, random,
count, `len(index)` and `memory_size`. Look up spaces/indexes by name or ID.
Name lookups resolve again on each operation, so they do not retain stale IDs
after a schema change. `box.schema_version()` exposes the host schema version.

Scalar keys become one-part keys; lists/tuples represent compound keys.
Queries return copied Python tuples and release host tuple references immediately.
Missing records return `None`; missing names raise `KeyError`.
Field numbers in update operations are zero-based, including host tuple updates.
`select()`/`iter()` default to ALL for an empty key and EQUAL otherwise;
override with `mode=box.IteratorType.GREATER_THAN_OR_EQUAL`, for example.
`random(seed)` forwards an explicit unsigned 32-bit seed.

`select()` materializes rows. Index iterators close at exhaustion or on failure;
use `with` for early exits. Resources also have an idempotent `close()`.
Garbage collection is a fallback, not a substitute for deterministic closing.

## Transactions and host resources

```python
with box.transaction(isolation=box.TransactionIsolation.READ_CONFIRMED):
    users.replace((1, "Alice", 20))
```

The context commits on success and rolls back on an exception. The runtime
requires a reschedulable `wasm.run` execution for transactions; a direct
`wasm.call` cannot start one. Transactions share the guest execution context;
nested transactions are rejected. Do not interleave transaction scopes across
Python coroutines. Explicit begin/commit/rollback, id/isolation,
set_isolation and make_synchronous are available on `box.transaction`.
For memtx, transactions spanning host calls require `memtx_use_mvcc_engine=true`;
otherwise Tarantool can abort them on a fiber yield. The SDK preserves that error.

```python
with box.key_def([box.KeyPart(0, box.FieldType.UNSIGNED)]) as definition:
    with box.tuple((1, "Alice")) as value:
        assert value[0] == 1
        assert definition.extract_key(value) == (1,)
        with box.tuple_format([definition]) as format:
            value.validate(format)
```

Host `Tuple` supports indexing, slices, JSON paths, byte size, conversion to
bytes/Python tuples, copy, update, upsert, validation and comparison. Update
returns another owned tuple; tuple upsert applies operations without inserting
into a space. `iter_fields()` exposes position, seek and rewind. Keep that
iterator in `with`: exhaustion preserves its cursor for rewind.
`value.format` returns an owned format which must also be closed.

`KeyDef` supports parts, length, copy, merge, partial/full key validation,
tuple validation and key extraction. `TupleFormat` supports copy;
`box.tuple_format()` obtains the default format. Copies own independent
references. Closed resources reject further access before reaching the host.
`copy.copy()` also acquires ownership for tuples, key definitions and formats.
Iterators and callbacks cannot be copied. Host resources cannot be deep-copied
or pickled; convert tuples to Python values for that purpose.

## Other interfaces

- `box.sequence(id)`: next(), current, set(value), reset(). WIT provides no
  sequence lookup by name.
- `box.session.current()` and `box.session[id]`: session objects with
  `send(header_dict, body_dict=None)` for iproto.
- `box.session.broadcast_lua(key, expression)`: the current host evaluates a
  Lua expression, so supply an explicit expression rather than serialized Python data.
- `box.error`: new(code, message, file=None, line=None), set, last, clear, format.
  Returned host failures raise `TarantoolError`, preserving code, message,
  error_type, file and line.
- `box.log.write(level, message, context=None)` supports every WIT log level.
  `logger.addHandler(box.log.handler())` connects Python logging.
- `box.metrics.uptime()` returns the host uptime.
- `box.msgpack.pack/unpack` preserve binary values, non-string map keys and
  opaque `msgpack.ExtType` values. Tarantool-specific extensions remain opaque.
  `from_json/to_json` expose the host's JSON conversions and their limitations.

## Lua callbacks

Copy the wheel's `twr_python_sdk/wit/lua-callback.wit` into
`wit/deps/twr-runtime/` and add
`import docs:adder/lua-callback@0.1.3;` to your world. For a world already in
`docs:adder@0.1.3`, put the file beside its root WIT and use `import lua-callback;`.
Tests check that this packaged contract matches [the runtime contract](../../../wit/adder.wit).
When also using SSL, put `ssl-bio.wit` in this same dependency directory:
both interfaces belong to the single `docs:adder@0.1.3` package.
The SDK does not register Lua closures; the embedding application does:

```lua
wasm.register_callback('echo', function(payload) return payload end)
```

```python
with box.lua_callback("echo") as echo:
    response = echo({"message": "hello"})
```

One call passes one MessagePack value and returns one value. Callback failures
raise `LuaCallbackError`. The host resolves the registered name on every call,
so replacement/unregistration affects an already-open callback.
The runtime converts requests into Lua values: `bytes` become Lua strings and
`None` becomes `nil`, which removes table entries. For binary responses use
Tarantool `varbinary`; for null entries use `box.NULL`. Plain Lua response strings
must be valid UTF-8, and an untyped empty Lua table becomes a Python list.

## Raw bindings and custom worlds

```python
import _tarantool

raw = _tarantool.database.space_by_name("users")
```

`_tarantool` exposes generated interface modules without conversion or ownership
management. Their original WIT result exceptions and retain/release rules apply.
For custom generated module names, configure `_tarantool` before accessing it,
or construct an isolated `box.Tarantool(bindings={"database": ..., "types": ...})`.
Provide all interfaces that the chosen operations use.

The installed-wheel Wasm fixture in `tests/components/tarantool` exercises all
interfaces; `tests/integration/tarantool_test.lua` runs them against Tarantool.
Native tests in `tests/unit/tarantool_test.py` cover cleanup and ownership errors.
