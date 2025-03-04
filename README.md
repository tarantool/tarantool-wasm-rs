# Tarantool WIT definitions

This repository contains WIT definition for Tarantool DBMS.

## Architecture Decision Record

Aside from enums, we opted to express every Tarantool type as either
record, resource or type alias. Choice of an expressing entity for each
type might seem arbitrary, but they are all aligned with the following
ideas:

Resource is an entity that lives outside of a WASM component. That means
its whole lifecycle is managed entirely by the embedder. These entities are
explicitly created and destroyed by the embedder on command from a WASM
component. Resource entities, unlike records, can have methods.

We made a decision to use resource semantics only for the iterators. One
might argue that the same semantics should apply to the spaces, indexes and
other types, since they are all handled by Tarantool and not by a WASM
component. That is a valid point, but a key difference between a space and
an iterator in regard to the ownesrhip semantics is that space is
manipulated by a `space_id` and is owned by Tarantool, while an iterator
is owned by an application and is manipulated directly.

Resource semantics also requires every resource to be freed after use,
which makes no sense for spaces or indexes, because we would have to free
a reference. Yet still, it makes perfect sense for iterators.

Final thing for consideration is object notation and methods. That feature
off resources looks very appealing to use for other Tarantool types, but it
has a few downsides to it. Every resource must be stored within a resource
table on the embedder side, and removed from there after free. Some of
our target languages have a notion of destructor and some of them don't.
That, in turn, results in making the user responsible for freeing the
resources from the resource table which is inconvenient, makes no sense in
comparison to Lua and leads to memory leaks.

With that in mind, the decision is to sacrifice methods in favor of
preserving the logic and memory safety.

Finally, we have type aliases. These are chosen for types that are
represented on the WASM side as opaque references. Unlike in records,
there is no generally useful underlying primitive to extract (like
`index_id`, for example), so we just wrap this reference into a nicely
named type.

## Requirements for generating bindings

### 1. Install `componentize-py`

```sh
sudo pip install componentize-py
```

### 2. Install `wit-bindgen`
Download the latest release of `wit-bindgen` from the official GitHub repository [WIT Bindgen Releases](https://github.com/bytecodealliance/wit-bindgen/releases)

## Generating Bindings

```sh
$ componentize-py --wit-path /path/to/repo --world tarantool bindings /output/dir
$ wit-bindgen c-sharp --runtime native-aot /path/to/repo

$ wit-bindgen
Usage: wit-bindgen <COMMAND>

Commands:
  markdown    This generator outputs a Markdown file describing an interface
  moonbit     Generates bindings for MoonBit guest modules
  rust        Generates bindings for Rust guest modules
  c           Generates bindings for C/CPP guest modules
  teavm-java  Generates bindings for TeaVM-based Java guest modules
  tiny-go     Generates bindings for TinyGo-based Go guest modules
  c-sharp     Generates bindings for C# guest modules
  help        Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version

```


## Useful links
[Introduction to component model.](https://component-model.bytecodealliance.org/introduction.html)
[Resource table docs.](https://docs.rs/wasmtime/latest/wasmtime/component/struct.ResourceTable.html)

