# Mapping WIT Types to Lua

This document describes how WIT interface types are mapped to Lua values when using `wasm` library.

Reference: [WIT Type System Specification](https://component-model.bytecodealliance.org/design/wit.html)

| WIT Type                  | Lua Representation             | Notes                                          |
| ------------------------- | -------------------------------| ---------------------------------------------- |
| `bool`                    | `true` / `false`               | Boolean literal                                |
| `s8`, `s16`, `s32`, `s64` | `number`                       | Signed integers                                |
| `u8`, `u16`, `u32`, `u64` | `number` or `uint64_t` cdata   | Values above `i64::MAX` are returned as LuaJIT cdata |
| `f32`, `f64`              | `number`                       | Floating point values                          |
| `char`                    | `string` (1-character)         | UTF-8 single character                         |
| `string`                  | `string`                       | UTF-8 string                                   |
| `list`                    | `table` (array)                | Homogeneous list                               |
| `option`                  | `T` or `nil`                   | `nil` represents `none`                        |
| `result`                  | `{true, ok}` or `{false, err}` | Errors may raise exceptions depending on guest |
| `tuple`                   | `table` (array)                | Fixed-length array                             |
| `record`                  | `table` (map)                  | Key-value table                                |
| `variant`                 | `table` (single key-value)     | E.g., `{ some = 42 }`                          |
| `enum`                    | `string`                       | One of the enum names                          |
| `flags`                   | `table` (array of strings)     | E.g., `{ "get", "put" }`                       |
| `own<resource>`           | `wasm.resource` userdata       | Owned handle; may be consumed or explicitly dropped |
| `borrow<resource>`        | `wasm.resource` userdata       | Borrowed for the duration of a session call    |

---

## Examples

All examples demonstrate arguments to WIT functions defined in the component.

### Type: `bool`

```wit
t-bool: func(x: bool);
```

```lua
component:t_bool(true)
component:t_bool(false)
-- Any non-boolean input (including strings, numbers, etc.) is interpreted as `false`.
-- Only `true` is preserved as `true`; all other values convert to `false`.
component:t_bool(-3)      -- `x` interpreted as false
component:t_bool("yes")   -- `x` interpreted as false
```

### Types: `s8`, `s16`, `s32`, `s64`

```wit
t-s8: func(x: s8);
t-s16: func(x: s16);
t-s32: func(x: s32);
t-s64: func(x: s64);
```

```lua
component:t_s8(-128)
component:t_s16(-12345)
component:t_s32(-100000)
component:t_s64(-123456789012345)
```

> JS components may not support precise `s64`/`u64` round-tripping due to `Number` limitations.

### Type: `u8`, `u16`, `u32`, `u64`

```wit
t-u8: func(x: u8);
t-u16: func(x: u16);
t-u32: func(x: u32);
t-u64: func(x: u64);
```

```lua
component:t_u8(255)
component:t_u16(65535)
component:t_u32(4294967295)
component:t_u64(123456789012345)
component:t_u64(require('ffi').cast('uint64_t', -1)) -- u64::MAX
```

### Type: `f32`, `f64`

```wit
t-f32: func(x: f32);
t-f64: func(x: f64);
```

```lua
component:t_f32(3.14)
component:t_f64(2.718281828459045)
```

### Type: `char`

```wit
t-char: func(x: char);
```

```lua
component:t_char("a")
component:t_char("z")
```

### Type: `string`

```wit
t-string: func(x: string);
```

```lua
component:t_string("hello")
component:t_string("Lua + WASM")
```

### Type: `list<T>`

```wit
t-list: func(x: list<string>);
```

```lua
component:t_list({})
component:t_list({"a", "b", "c"})
```

### Type: `option<T>`

```wit
t-option: func(x: option<u8>);
```

```lua
-- Some guest languages (like C) may return 0 to represent None/nil.
component:t_option(nil)
component:t_option(42)
```

### Type: `result<T, E>`

```wit
t-result: func() -> result<u8, string>;
```

```lua
local ok, res = unpack(component:t_result())
if ok then
  -- res has type u8
  print("Success:", res)
else
  -- res has type string
  print("Error:", res)
end
```

> **Limitations**:
> `result<T, E>` **can only be used as a return type**.
> It **is not supported** as a function parameter in the current Tarantool WASM implementation.

> **Notes**:
> Also it's important to understand that the behavior of `result<T, E>` in WebAssembly
> components can vary significantly depending on the source language used to implement the component.
> Some languages raise exceptions natively (like Python), while others return errors as structured
> values (like C). This affects how the Tarantool host receives and handles these results.
> Always use `pcall(m:t_result(...))` or wrap calls in a helper if you're unsure whether the component
> may raise exceptions. Use direct pattern matching only when you're confident the backend uses safe
> return-based error handling.

### Type: `tuple<T1, T2, ...>`

```wit
t-tuple: func(x: tuple<bool, string>) -> tuple<bool, string>;
```

```lua
component:t_tuple({true, "abc"})
```

### Type: `record`

```wit
record r {
  x: u32,
  y: string,
}

t-record: func(x: r);
```

```lua
component:t_record({x = 1, y = "value"})
```

### Type: `variant`

```wit
variant v {
  none,
  any(bool),
  restricted(list<string>),
}

t-variant: func(x: v);
```

```lua
component:t_variant({none = true})
component:t_variant({any = true})
component:t_variant({restricted = {"a", "b"}})
```

For a payload-less case, the input uses `{ none = true }`. The current return
conversion produces an empty table (`{}`), because Lua tables cannot retain a
key whose value is `nil`.

### Type: `enum`

```wit
enum e {
  red,
  green,
  blue,
}

t-enum: func(x: e);
```

```lua
component:t_enum("red")
component:t_enum("blue")
```

### Type: `flags`

```wit
flags f {
  read,
  write,
  execute,
}

t-flags: func(x: f) -> f;
```

```lua
component:t_flags({})
component:t_flags({"read", "execute"})
```

### Resources

WIT resources require a persistent component instance. `wasm.bind(module)`
opens and owns a session automatically. The explicit `open_session`,
`session_call`, and `session_batch` APIs use the same resource representation.
The one-shot `call` and `batch` APIs do not support resource handles.

```wit
resource counter {
  constructor(initial: u32);
  get: func() -> u32;
  add: func(delta: u32) -> u32;
  sum: static func(first: borrow<counter>, second: borrow<counter>) -> u32;
}

consume: func(value: counter) -> u32;
```

```lua
local module = wasm.load('./counters.wasm')
local component = wasm.bind(module)
local counters = component['example:counters/api@0.1.0']

local first = counters.counter.new(20)
local second = counters.counter.new(22)

assert(first:get() == 20)
assert(first:add(2) == 22)
assert(counters.counter.sum(first, second) == 44)

-- `consume` takes own<counter>; `first` is invalid after this call.
assert(counters.consume(first) == 22)

-- Explicit cleanup is deterministic. Remaining live resources are also
-- released when their session closes.
second:drop()
wasm.drop(module)
```

The userdata methods `type()`, `owned()`, and `is_alive()` expose local handle
metadata. `drop()` returns `true` when it releases a live handle and `false`
after an earlier explicit drop. Lua garbage collection schedules a fallback
drop, but applications should call `drop()` when release timing matters.

Resources are session-affine. Passing a handle to a function bound to another
module session raises an error before entering the component. Passing a handle
as `own<T>` transfers ownership to the guest; a subsequent use reports that the
resource is no longer available. `borrow<T>` calls leave the handle in the
session resource table.

Resources may appear inside lists, maps, options, tuples, records, variants,
and results. The conversion walks those values recursively and preserves the
same session and ownership checks for nested handles.

Imported host resources, including Tarantool iterators and WASI resources,
continue to use Wasmtime's host resource table. Their TX-thread constraints are
described in [thread-safety.md](thread-safety.md).

See: [WIT Spec - Resource](https://component-model.bytecodealliance.org/design/wit.html#resources)
