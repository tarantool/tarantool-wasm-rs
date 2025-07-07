# tarantool-wasm

### Main Repositories:

1. **[tarawasm](https://github.com/mandesero/tarawasm)** – A CLI utility for working with WebAssembly (WASM) modules written in guest languages. Currently supports Python, Go, JavaScript, C/C++ and Rust.
2. **[tarantool-wasm-rs](https://github.com/mandesero/tarantool-wasm-rs)** – A library for WASM integration in Tarantool.
3. **[tarantool-wit](https://github.com/mandesero/tarantool-wit)** – Interface definitions (WIT) for working with Tarantool in WASM.

---

## CLI: Using tarawasm

### Installation via Docker

```sh
$ docker pull mandeser0/tarawasm:latest
```

### Optional alias for convenience:

```sh
$ alias tarawasm='docker run --rm -v "$PWD":/work -w /work mandeser0/tarawasm'
```

To start using the utility, you need precompiled WASM interfaces that will be used by your WASM component.

Example components can be found at:
**[https://github.com/mandesero/tarawasm/tree/master/examples](https://github.com/mandesero/tarawasm/tree/master/examples)**
(see `<lang>/docs:adder@0.1.0.wasm`)

### Basic Example:

```sh
$ mkdir my-component && cd my-component
$ cp <path-to>/docs:adder@0.1.0.wasm .
$ tarawasm init --lang python --wasm-file docs:adder@0.1.0.wasm adder
$ tarawasm bind
$ tarawasm build
# This will produce `adder.wasm`, which can be executed using any WASM runtime such as `wasmtime` or `tarantool-wasm`.
```

The `tarawasm` CLI acts as a wrapper around various WASM tools depending on the guest language. You can override the default behavior and specify custom options for `bind` and `build`.

---

### Language-specific Examples

**Python:**

```sh
$ tarawasm init --lang python --wasm-file docs:adder@0.1.0.wasm adder
$ tarawasm bind --help
# Output:
A utility to convert Python apps into Wasm components

Usage: componentize-py [OPTIONS] <COMMAND>
Options:
  -d, --wit-path <WIT_PATH>   File or directory containing WIT document(s)
...
```

**Rust:**

```sh
$ tarawasm init --lang rust --wasm-file docs:adder@0.1.0.wasm adder
$ tarawasm bind --help
# Output:
Just update the generated bindings.

The generated bindings are created automatically by subcommands like `cargo component build`. Use `cargo component bindings` if you only want to generate the bindings.

Usage: cargo component bindings [OPTIONS]
Options:
  -q, --quiet   Do not print log messages
...
```

By default, all necessary options for `bind` and `build` are preconfigured, but you may override them manually.

---

## WASM Library Integration

### Dependencies:

* **Rust**

```sh
$ curl https://sh.rustup.rs -sSf | sh
```

* **wkg**

```sh
$ cargo install wkg
```

* **just**

```sh
$ cargo install just
```

### Clone and build the library:

```sh
$ git clone git@github.com:mandesero/tarantool-wasm-rs.git
$ cd tarantool-wasm-rs
$ git checkout mandesero/wit-impl
$ just
```

---

## Building Patched Tarantool (WASM-compatible with fiber stack disabled)

```sh
$ git clone https://github.com/mandesero/tarantool
$ cd tarantool
$ git checkout wasm

$ mkdir build && cd build
$ cmake .. -DCMAKE_BUILD_TYPE=Debug -DENABLE_BACKTRACE=ON -DTARANTOOL_WASM=ON
$ make -j
```

---

## Running a WASM Component in Tarantool

Example Lua script:
(see more examples at: [https://github.com/mandesero/tarantool-wasm-rs/tree/mandesero/wit-impl/examples](https://github.com/mandesero/tarantool-wasm-rs/tree/mandesero/wit-impl/examples))

```lua
-- run.lua
local wasm = require('my_module')
local wasm_module_path = ...

log.info("Load WASM module...")
local module = wasm.load(wasm_module_path)

log.info("Run WASM module...")
local handle = wasm.run(module)

wasm.join(handle)
log.info("WASM module finished...")
```

Run with:

```sh
$ tarantool run.lua <path-to-wasm>
```

## Using Exported Functions from a Component

This guide demonstrates how to call exported WebAssembly functions from Lua in Tarantool using the `wasm` components.

```sh
$ mkdir my-component && cd my-component
$ cp <path-to>/docs:adder@0.1.0.wasm .
$ tarawasm init --lang python --wasm-file docs:adder@0.1.0.wasm adder
```

Edit `wit/world.wit` to define your exports:

```wit
world adder {
  ...
  export add: func(x: s32, y: s32) -> s32;
  export greet: func(name: string) -> string;
  export concat: func(items: list<string>) -> string;
  export split: func(str: string, sep: string) -> list<string>;
  ...
}
```

Generate bindings:

```sh
$ tarawasm bind
```

Implement the exported functions in Python (`main.py`):

```python
from wit_world import exports
from typing import List

class IncomingHandler(exports.IncomingHandler):
    def handle(self, request, response_out):
        pass

class Run(exports.Run):
    def run(self) -> None:
        print("Hello from Python WASM!")

class WitWorld():
    def add(self, x: int, y: int) -> int:
        return x + y

    def greet(self, name: str) -> str:
        return f"Hello, {name.capitalize()}!"

    def concat(self, items: List[str]) -> str:
        return "-".join(items)

    def split(self, str: str, sep: str) -> List[str]:
        return str.split(sep)
```

Build and strip the component:

```sh
$ tarawasm build
$ tarawasm strip adder.wasm  # Optional: reduce the component size by removing debug and custom sections
```

---

### Running in Tarantool

```
$ tarantool -i

Tarantool 3.5.0-entrypoint-62-g40dc5c548d
type 'help' for interactive help
tarantool> wasm = require('wasm')
---
...

tarantool> m = wasm.load("adder.strip.wasm")
---
...

tarantool> r = wasm.call(m, "add", 228, 1337)
---
...

tarantool> print(r, type(r))
1565	number
---
...

tarantool> wasm.call(m, "greet", "alice")
---
- Hello, Alice!
...

tarantool> wasm.call(m, "concat", {"s1", "s2", "s3"})
---
- s1-s2-s3
...

tarantool> wasm.call(m, "split", "aaaa bbb cc", " ")
---
- - aaaa
  - bbb
  - cc
...

tarantool> wasm.call(m, "split", "aaaa bbb cc", "b")
---
- - 'aaaa '
  -
  -
  - ' cc'
...

tarantool>
```