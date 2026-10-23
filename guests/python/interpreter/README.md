# Python interpreter component

`adder.wasm` exports `exec(code)` and MessagePack `call(func, args)` for
`wasm.load(path, {interpreter = true})`. It imports
`tarantool:tarantool@0.2.0` and uses the installed Python SDK.

```sh
# From repository root:
just python-sdk
# Build the interpreter and test components with the SDK README Docker command.
# Interpreter only, with the matching build tools installed and wheel built:
sh guests/python/interpreter/build.sh /tmp/python-interpreter.wasm
```

`requirements-build.txt` pins MessagePack and optional HTTP packages.
`main.py` declares modules needed by dynamically executed code. The SDK installs
ports before loading it; this interpreter package set is not an SDK dependency.

Ordinary Python exceptions preserve shared globals; Wasm traps recreate the
instance. Configure network permissions, preopens, memory and fuel with
`wasm.load`. See [SDK](../README.md) for custom application builds and
[SSL](../ports/ssl/README.md) for compatibility limits.
