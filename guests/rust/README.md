# Rust guest SDK

`tarantool-wasm` runs **inside tarantool-wasm-rs**. It is not a network client.
It uses the released `tarantool:tarantool@0.2.0` contract, with the version and
digest recorded in `wkg.lock`.

## Build the SDK

Install Rust 1.96.0 or newer, wkg and just. From the repository root:

```sh
just rust-sdk
```

This configures the Tarantool registry, replaces downloaded WIT dependencies,
builds the library and creates a verified package at
`guests/rust/target/package/tarantool-wasm-0.1.0.crate`.

To build the host and SDK together, with the host's build dependencies installed
(see the [project README](../../README.md)), use:

```sh
just all rust-sdk
```

The equivalent manual SDK build does not require just:

```sh
bash scripts/add_tarantool_registry.sh
cd guests/rust
rm -rf wit/deps
wkg wit fetch --type wasm
cargo build --locked
cargo package --locked --allow-dirty
```

Clear `wit/deps` before fetching to remove dependencies left by another WIT
version. `wkg.lock` is maintained with the SDK; changing the WIT version requires
updating that lock explicitly. `--allow-dirty` lets Cargo include the downloaded,
Git-ignored WIT dependency in the package.

Cargo does not fetch WIT. When using a repository checkout as a path dependency,
prepare it with either process above before running Cargo in your application.
The `.crate` already includes WIT and needs no wkg or network access for WIT.

## Use the SDK

Extract the `.crate` and point your dependency at its directory, or use the
prepared repository checkout:

```toml
[dependencies]
tarantool-wasm = { path = "../tarantool-wasm-rs/guests/rust" }
```

```rust,no_run
use tarantool_wasm::{Space, IteratorType, Result};

fn users() -> Result<()> {
    let users = Space::by_name("users")?.expect("schema must exist");
    let row = users.insert(&(1u64, "Alice"))?;
    let (id, name): (u64, String) = row.decode()?;
    for row in users.primary().scan(&Vec::<u8>::new(), IteratorType::All)?.take(10) {
        let user: (u64, String) = row?.decode()?;
        println!("{user:?}");
    }
    Ok(())
}
```

## Values and resources

Keys, tuples and update operations must serialize as arrays. Use Rust tuples,
arrays, or vectors; structs use Serde's compact array representation. Single
keys use `(id,)`; empty keys use an empty vector. Database update positions are
zero-based, e.g. `index.update(&(1u64,), &[("=", 1, "Bob")])?`.
Space/index IDs are resolved at lookup; repeat name lookup after schema changes.
Missing lookups return `Ok(None)`; errors retain all host fields in `Error::Host`.

`Tuple` owns one host reference and releases it on `Drop`; `Clone` retains a new
reference. `Scan` is a fused `Iterator<Item = Result<Tuple>>`; exhaustion, errors
and dropping it release the host iterator. Use normal iterator adaptors for
limits/offsets. Tuple owners cannot move across threads. Fatal traps and aborts
cannot run Rust destructors.

`raw` exposes **every** platform interface, including transactions, sessions,
sequences, diagnostics, logging, metrics, key definitions and tuple formats.
Raw record handles require the matching retain/release functions; generated
WIT resource objects own and drop themselves. Raw tuple update positions retain
the host API's conventions. The SDK creates no schema or runtime policy.
Transactions require reschedulable `wasm.run` and memtx MVCC; see the host docs.

## MessagePack

`msgpack::{pack, unpack}` uses Serde directly and rejects trailing bytes. `Value`
retains arbitrary/duplicate map keys, binary and unknown signed extension types.
Use `Value::Binary` for bytes (ordinary Rust vectors serialize as arrays).
Typed extension values implement Serialize/Deserialize:

- `Decimal { digits, scale, negative }`: exact coefficient, up to 38 digits.
- `Uuid([u8; 16])`: network byte order.
- `Datetime { seconds, nanoseconds, tz_offset, tz_index }`: no timezone/nanosecond loss.
- `Interval`: signed fields and `Adjust::{Excess, None, Last}`.
- `ErrorValue`: numeric attribute map with stack at key 0, preserving future fields.

Dynamic `Value::Ext` preserves payloads without validation; decode into a typed
extension to validate its wire representation. `ErrorValue` validates the numeric
attribute map and stack envelope; frame contents remain uninterpreted. Host limits
may be narrower.

## Build application components

Build application components with `cargo build --target wasm32-wasip2 --release`.
The application owns its exports and calls `wit_bindgen::generate!` for its own
world, as in `tests/component`. SDK imports are linked from this crate. No
application world name, callback API or WASI networking dependency is imposed.
Strip the final component with `wasm-tools strip`, then `wasm-tools validate`.

## Run tests

Verified tools: Rust 1.96.0 with `wasm32-wasip2`, wit-bindgen 0.58.0,
wasm-tools 1.261.0 and Tarantool 3.7. With WIT already prepared, native codec tests
need only Rust; native calls to host imports are unsupported. After building the
SDK, run from the repository root:

```sh
cargo test --locked --manifest-path guests/rust/Cargo.toml
cargo build --locked --manifest-path guests/rust/tests/component/Cargo.toml --target wasm32-wasip2 --release
component=guests/rust/tests/component/target/wasm32-wasip2/release/rust_sdk_test.wasm
wasm-tools strip "$component" -o "$component"
wasm-tools validate "$component"
just build
bash guests/rust/tests/test-runtime.sh
```

The [SDK linters workflow](../../.github/workflows/sdk-lint.yml) checks
formatting and runs Clippy for Rust code under `guests/` on every pull request.
The [SDK workflow](../../.github/workflows/sdk.yml) runs the build,
tests and package verification. It uploads a `.crate`
archive containing the standalone SDK and fetched WIT contract. Extract it and
use its directory as the dependency path above. Pull requests require the
`sdk-ci` or `full-ci` label; the workflow can also be started manually.
