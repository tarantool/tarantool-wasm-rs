# Python interpreter component

`adder.wasm` embeds CPython and exposes the interpreter contract expected by
`wasm.load(path, { interpreter = true })`:

```wit
export exec: func(code: string) -> result<_, string>;
export call: func(func: string, args-msgpack: list<u8>) -> result<list<u8>, string>;
```

## Rebuilding

The build requires Python 3.10 or newer with `pip`, plus exactly
`wasm-tools 1.245.1`. The Python build dependencies are pinned in
`requirements-build.txt`.

```sh
cargo install wasm-tools --version 1.245.1 --locked
./python-interpreter/build.sh
```

Pass an output path to keep the checked-in fixture unchanged:

```sh
./python-interpreter/build.sh /tmp/python-interpreter.wasm
```

The component imports the Tarantool WIT interfaces declared in
`wit/adder.wit`. Python modules that executed code may import must also be
imported at the top level of `main.py`, because `componentize-py` resolves
dependencies while building the component.

Normal Python exceptions are returned through the WIT `result` and do not
destroy interpreter state. Exceptions escaping the component or host/runtime
traps are treated as an unsafe instance state and cause the Rust worker to
reinstantiate the component.
