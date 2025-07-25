# tarantool-wasm

A library for integrating WebAssembly (WASM) components into Tarantool.

## Main Repositories

1. **[tarawasm](https://github.com/mandesero/tarawasm)** – A CLI tool for building and managing WebAssembly components written in guest languages. Currently supports Python, Go, JavaScript, C/C++, and Rust.

2. **[tarantool-wit](https://github.com/mandesero/tarantool-wit)** – Interface definitions (WIT) for exposing Tarantool APIs to WASM modules.

3. **[tarantool-wasm-samples](https://github.com/mandesero/tarantool-wasm-samples)** – Examples demonstrating how to use Tarantool with WASM.

4. **[tarantool-wasm-module](https://github.com/mandesero/tarantool/tree/wasm-module)** *(Work in Progress)* – A patched Tarantool build with native WASM runtime integration and Lua bindings.

See [developer\_guide.md](docs/developer_guide.md) for details on architecture and contribution workflow.

---

## Table of Contents

* [Building the Patched Tarantool](#building-the-patched-tarantool-wasm-compatible-with-fiber-signal-stack-disabled)
* [Building the WASM Runtime Library](#building-the-wasm-runtime-library)
* [Working with tarawasm](#working-with-tarawasm)
* [Manual WIT Interface Definition](#manual-wit-interface-definition)
* [Using in Tarantool](#using-in-tarantool)

  * [Option 1: Using wasm.so directly](#option-1-using-wasmso-directly)
  * [Option 2: Using luawasm wrapper](#option-2-using-luawasm-wrapper)

---

## Building the Patched Tarantool (WASM-Compatible with Fiber Signal Stack Disabled)

To enable WASM integration, build Tarantool with `TARANTOOL_WASM=ON`:

```sh
git clone https://github.com/mandesero/tarantool
cd tarantool
git checkout wasm-module

mkdir build && cd build
cmake .. -DCMAKE_BUILD_TYPE=Debug -DENABLE_BACKTRACE=ON -DTARANTOOL_WASM=ON
make -j
```

---

## Building the WASM Runtime Library

Clone the runtime:

```sh
git clone https://github.com/mandesero/tarantool-wasm-rs.git
cd tarantool-wasm-rs
git checkout mandesero/wit-impl
```

Install Rust (see [rust-lang.org](https://www.rust-lang.org/tools/install)):

```sh
curl https://sh.rustup.rs -sSf | sh
```

Install the `just` build tool (see [casey/just](https://github.com/casey/just)):

```sh
# Faster
sudo apt install just

# Or slower via Cargo
cargo install just
```

Install `wkg`, a WIT package manager (see [wasm-pkg-tools](https://github.com/bytecodealliance/wasm-pkg-tools)):

```sh
cargo install wkg
```

Patch `wkg` config with the local path to `tarantool-wit` interfaces:

```sh
just patch-wkg-config
```

Build the library:

```sh
just
```

Then place the compiled `.so` file in a location accessible to Tarantool:

```sh
sudo ln -s <path-to>/target/debug/libtarantool_wasm_rs.so /usr/local/lib/tarantool/wasm.so
```

---

## Working with `tarawasm`

Install `tarawasm` (see [tarawasm repo](https://github.com/mandesero/tarawasm)).

To initialize a new WASM component, you need a precompiled `.wasm` interface file. Build it using:

```sh
wkg wit build
```

This will produce something like `docs:adder@0.1.1.wasm`, where `adder` is the WIT world name.

Start a new component:

```sh
mkdir my-component && cd my-component
cp ../docs:adder@0.1.0.wasm .
tarawasm init --lang python --wasm-file docs:adder@0.1.0.wasm adder
tarawasm bind
```

This generates all required boilerplate. See full examples in [tarantool-wasm-samples](https://github.com/mandesero/tarantool-wasm-samples).

---

## Manual WIT Interface Definition

You may define your own `.wit` file in the `wit/` directory of your project. For example:

```wit
package example:<your-world>@0.1.0;

world <your-world> {
  // Optional Tarantool APIs
  import tarantool:tarantool/types@0.1.1;
  import tarantool:tarantool/say@0.1.1;
  ...

  // Optional standard WASI interfaces
  import wasi:cli/environment@0.2.3;
  import wasi:cli/exit@0.2.3;
  ...

  // Optional WASI run interface (only needed for fiber execution)
  export wasi:cli/run@0.2.3;

  // Custom exports
  export add: func(x: s32, y: s32) -> s32;
}
```

Build the interface:

```sh
wkg wit build
```

Resulting in `example:<your-world>@0.1.0.wasm`. Then:

```sh
tarawasm init --lang python --wasm-file example:<your-world>@0.1.0.wasm <your-world>
tarawasm bind
```

Implement your logic:

**main.py**

```py
from wit_world import exports
from typing import List

class Run(exports.Run):
    def run(self) -> None:
        print("Hello from Python WASM!")

class WitWorld():
    def add(self, x: int, y: int) -> int:
        return x + y
```

Build the component:

```sh
tarawasm build
```

---

## Using in Tarantool

### Option 1: Using `wasm.so` directly

```sh
$ tarantool -i

Tarantool 3.5.0-entrypoint-62-g40dc5c548d
type 'help' for interactive help
tarantool> wasm = require('wasm')
---
...

tarantool> m = wasm.load("<path-to-your-world.wasm>")
---
...

tarantool> wasm.call(m, "add", 1, 2)
---
- 3
...

tarantool> handle = wasm.run(m)
---
...

Hello from Python WASM!

---
...

tarantool> wasm.join(handle)
---
- true
...
```

---

### Option 2: Using `luawasm` wrapper

```sh
$ tarantool -i

Tarantool 3.5.0-entrypoint-62-g40dc5c548d
type 'help' for interactive help
tarantool> wasm = require('wasm')
---
...

tarantool> lwasm = require('luawasm')
---
...

tarantool> m = lwasm:new({ dir = "."})
---
...

tarantool> m:help()
Exported functions:

add(x: s32, y: s32) -> s32

---
...

tarantool> m:add(1, 2)
---
- 3
...

tarantool> m:run()
---
...

tarantool> Hello from Python WASM!

---
...

tarantool> m:join()
---
- true
...
```
