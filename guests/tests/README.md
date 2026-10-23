# SDK compatibility tests

`compatibility_test.lua` checks all 25 ordered pairs of Rust, C#, Go,
JavaScript and Python SDKs, including each SDK with itself. It passes values
through the producer's MessagePack codec, stores the resulting tuple in
Tarantool, and passes the stored values through the consumer's codec.

Cases cover nulls, booleans, UTF-8, binary data, nested arrays and maps, empty
containers, 64-bit integer limits, integers beyond JavaScript's safe range,
decimal, UUID, datetime, interval and error values. Error metadata is compared
by value; map ordering is not significant.

The suite reuses the installed-package components built by each SDK's existing
tests. Python uses its SDK-enabled interpreter. No separate peer components or
guest compilers are needed to run the suite.

Build the components using each SDK's README, then run from the repository root
with the native module installed:

```sh
PATH="$PWD/.rocks/bin:$PATH" luatest -v -l luatest.log \
  guests/tests/compatibility_test.lua
```

The SDK CI compatibility job downloads these components and the shared host
artifact after all five SDK workflows succeed. It runs this same test file
without rebuilding the SDKs or host.
