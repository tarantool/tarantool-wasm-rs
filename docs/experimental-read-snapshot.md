# Experimental read snapshot contract

`docs:adder/experimental-read@0.1.3` promises that a successful `open()` fixes
one committed snapshot before returning. Every later `get`, `count`, `select`,
and `iterator.next` in the context reads that snapshot. A guest must abort its
SQL statement if `open()` returns an error; ordinary transaction reads are not
a safe fallback.

The runtime uses a Tarantool application read view for this guarantee. On the
tested Tarantool `3.9.0-entrypoint-63-g0bf19dcc6` build, a memtx view retained
the old tuple after a later commit. The same view omitted a Vinyl user space.
`open()` therefore checks every existing user space and index against the view
and returns `box-error` with a `snapshot unavailable:` message if the view is
incomplete. It also fails if the application read-view API is absent. This is a
runtime capability check, not a blanket claim that Tarantool CE provides
application read views. Tarantool's [read-view documentation][read-views]
describes availability and engine limitations separately from transaction
isolation.

A `READ_CONFIRMED` transaction is insufficient: the tested baseline allowed a
commit after `open()` to appear in the first `get`. A full `count` happened to
keep later writes out in one scenario because it created a read dependency;
it did not establish a database-wide snapshot at `open()`.

## Proposed next WIT version

Version `docs:adder@0.1.4` should make the unavailable case structured at the
`open` call, so PostgreSQL guests can reject unsupported semantics without
parsing a Tarantool error message:

```wit
variant snapshot-unavailable {
  read-view-api-absent,
  unsupported-space(string),
  unsupported-index(string),
}

variant read-open-error {
  unavailable(snapshot-unavailable),
  tarantool(box-error),
}

open: func() -> result<read-context, read-open-error>;
```

An `unavailable` result must not return a context or defer the failure to the
first read. Guests should abort the statement on either error variant. A
capability query alone would leave a race with schema changes before `open()`.

[read-views]: https://www.tarantool.io/en/doc/latest/reference/reference_lua/box_read_view/open/
