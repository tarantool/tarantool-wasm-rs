# JavaScript SDK for Tarantool-Wasm-Rs

`twr-js-sdk` is an ESM library for JavaScript WebAssembly components running in
Tarantool-Wasm-Rs. It wraps all interfaces of `tarantool:tarantool@0.2.0`: spaces,
indexes, tuples, key definitions, transactions, sequences, sessions, diagnostics,
logging, metrics and MessagePack. CRUD and Lua callbacks accept JavaScript values
and release temporary host tuples automatically. TypeScript declarations ship
with the package.

The SDK is a library used by your component. Your application supplies its WIT
world and exports, and the Tarantool application creates spaces, indexes and
callback registrations. Operations absent from WIT, including schema creation,
are not supplied by the SDK. Standard library ports, TLS wrappers and a separate
JavaScript interpreter are outside this stage.

## Install and write a component

Create an installable archive from this checkout:

```sh
cd guests/js
npm ci --ignore-scripts
mkdir -p dist
npm pack --pack-destination dist
```

In your component project, install the archive and save it as a dependency:

```sh
npm init -y
npm pkg set type=module
npm install /path/to/twr-js-sdk-0.1.0.tgz
```

Spaces and indexes accept names or numeric IDs. Names are resolved on every
operation, so a saved wrapper follows schema changes. CRUD returns decoded tuple
arrays; a lookup that finds no row returns `null`. Update operation field numbers
and tuple field access use zero-based JavaScript indexes.

## WIT and tarawasm

Create `wit/world.wit` with your exports. WIT cannot express arbitrary JavaScript
values: the example describes its fixed tuple as a WIT record. ComponentizeJS
expects a JavaScript object for that record, so adapt the exported functions as
shown below; database tuples remain arrays inside the SDK.

```wit
package example:users@0.1.0;

interface api {
  record user { id: u32, name: string }
  create-user: func(id: u32, name: string) -> user;
  find-user: func(id: u32) -> option<user>;
}

world users {
  include tarantool:tarantool/guest@0.2.0;
  export api;
}
```

For this world, use the following `main.js`:

```javascript
import {box} from 'twr-js-sdk';

export const api = {
  createUser(id, name) {
    const row = box.space('users').insert([id, name]);
    return {id: row[0], name: row[1]};
  },
  findUser(id) {
    const row = box.space('users').index('primary').get([id]);
    return row === null ? undefined : {id: row[0], name: row[1]};
  },
};
```

Fetch `tarantool:tarantool@0.2.0` from the released tarantool-wit registry. In this
repository, `scripts/add_tarantool_registry.sh` configures the registry for `wkg`;
use the same registry configuration in your build environment. Applications can
also include the packaged `twr:js-sdk/guest@0.1.0` world after copying
`node_modules/twr-js-sdk/wit/guest.wit` into their WIT dependencies. Neither approach
requires the host's root `world adder` or its exports.

Create `tarawasm.json`:

```json
{
  "language": "js",
  "world": "users",
  "wit": {"path": "wit", "package": "example:users@0.1.0"},
  "source": "main.js",
  "output": "users.wasm"
}
```

Build with the published `mandeser0/tarawasm:latest` image. Keep `node_modules`
inside the mounted project; tarawasm bundles npm dependencies automatically.

```sh
docker pull mandeser0/tarawasm:latest
docker run --rm -v "$PWD:/workspace" -w /workspace \
  mandeser0/tarawasm:latest all
docker run --rm -v "$PWD:/workspace" -w /workspace \
  mandeser0/tarawasm:latest strip users.wasm
mv users.strip.wasm users.wasm
docker run --rm -v "$PWD:/workspace" -w /workspace \
  --entrypoint wasm-tools mandeser0/tarawasm:latest validate users.wasm
```

Load the component from a Lua application after creating its schema:

```lua
local wasm = require('wasm')
box.cfg{}
local users = box.schema.space.create('users')
users:create_index('primary')
local component = wasm.load('users.wasm')
local api = 'example:users/api@0.1.0::'
local user = wasm.call(component, api .. 'create-user', 1, 'Alice')
assert(user.name == 'Alice')
wasm.drop(component)
```

## Build directly with Jco or ComponentizeJS

With Node.js and a configured `wkg` registry, use the same installed package,
`main.js` and WIT world:

```sh
npm install --save-dev @bytecodealliance/jco@1.37.0 \
  @bytecodealliance/componentize-js@0.23.0
npm pkg set 'overrides.@bytecodealliance/componentize-js.@bytecodealliance/preview2-shim=0.17.8'
npm install
wkg fetch --type wasm
npx --no-install jco componentize main.js --bundle --wit wit \
  --world-name users -o users.wasm --disable clocks http random fetch-event
wasm-tools strip users.wasm -o users.strip.wasm
mv users.strip.wasm users.wasm
wasm-tools validate users.wasm
```

`--bundle` is required for npm imports. The ComponentizeJS dependency uses
preview2-shim 0.17.8 in the verified toolchain; the override preserves its
filesystem preinitialization behavior. When using the ComponentizeJS CLI
itself, bundle the ESM source first and leave WIT import specifiers external:

Create `rolldown.config.js`:

```javascript
export default {
  input: 'main.js',
  external: (id) => id.startsWith('tarantool:') || id.startsWith('docs:adder/'),
  output: {file: 'bundled.js', format: 'esm'},
};
```

```sh
npm install --save-dev rolldown@1.2.13
npx --no-install rolldown --config rolldown.config.js
npx --no-install componentize-js bundled.js --wit wit --world-name users \
  -o users.wasm --disable clocks http random fetch-event
wasm-tools strip users.wasm -o users.strip.wasm
mv users.strip.wasm users.wasm
wasm-tools validate users.wasm
```

Verified Docker/direct builds used Node.js 24.21.0; local tarawasm builds and
unit/declaration checks used Node.js 25.9.0. Both build profiles used Jco 1.37.0,
ComponentizeJS 0.23.0,
preview2-shim 0.17.8, Jco standard adapters 0.7.0, Rolldown 1.2.13, `wkg` 0.16.1 and
`wasm-tools` 1.261.0. Declaration checks used TypeScript 5.9.3.
`sdk-manifest.json` records compatibility versions. Docker examples use
tarawasm's `latest` tag; CI uses the latest Git `master` with local tools.
WIT dependency locks are kept separately.

## Lua callbacks and raw bindings

Callbacks are optional. Add this import to your world:

```wit
import docs:adder/lua-callback@0.1.3;
```

Copy `node_modules/twr-js-sdk/wit/runtime/host.wit` to
`wit/deps/adder/host.wit`. It contains the existing callback and zlib interfaces
in one `docs:adder@0.1.3` package. Importing the base SDK does not require either
interface.

```javascript
import {open, withCallback} from 'twr-js-sdk/lua-callback';

export function lookup(value) {
  return withCallback('lookup', (callback) => callback.call(value));
}

export function repeatedLookup(value) {
  const callback = open('lookup');
  try {
    return callback.call(value);
  } finally {
    callback.close();
  }
}
```

The Lua application registers `lookup` through the runtime callback API before
invocation. The SDK encodes one argument value and decodes one result value;
callback failures become `LuaCallbackError`. `call()` without an argument sends
`null`; explicitly passing `undefined` fails. Lua callbacks must preserve
MessagePack array/map distinctions, `box.NULL` and binary values when forwarding
values.

For unchanged WIT arguments and results, use `twr-js-sdk/_tarantool`:

```javascript
import * as raw from 'twr-js-sdk/_tarantool';
```

Host zlib is available only through `twr-js-sdk/_zlib`: `compress`, `decompress`,
`crc32` and `adler32`. Add `import docs:adder/zlib@0.1.3` to your world and use the
same extra WIT dependency. This raw API preserves the host ABI, including its
failure conventions, and does not validate or convert arguments. ComponentizeJS
also supplies `CompressionStream` and `DecompressionStream` for deflate, gzip and
deflate-raw, so stream compression does not require this host interface.

## Values, resources and transactions

JavaScript values are encoded directly as MessagePack. `null` is nil;
`undefined`, sparse arrays, unsafe integer `number` values and cyclic objects are
rejected. Binary values use `Uint8Array`. Decoded integers outside the exact
`number` range use `bigint`; pass large integers as `bigint` too. Arrays remain
arrays. Plain objects and `Map` encode as maps; maps with only string keys decode
to plain objects, and maps with non-string keys decode to `Map`.

Use `UUID`, `Decimal`, `Datetime`, `Interval` and `ErrorValue` for Tarantool
extensions. Decimal digits and scale are preserved without conversion to
`number`; datetime preserves seconds, nanoseconds and timezone fields without
conversion to `Date`. Unknown extensions become `Extension(type, Uint8Array)`.
`pack` and `unpack` are available from the base package and `twr-js-sdk/codec`.
Host `box.msgpack.fromJson` and `toJson` are explicit WIT helpers; SDK operations
do not use JSON as an intermediate representation.

Ordinary CRUD needs no resource cleanup. `box.tuple`, `box.keyDef`,
`box.tupleFormat`, callbacks and cursors return owned resources. Each returned
handle, including copies and tuple formats, needs an idempotent `close()` or
`usingResource` call:

```javascript
import {box, usingResource} from 'twr-js-sdk';

export function inspectTuple() {
  return usingResource(box.tuple([1, 'Alice']), (tuple) => tuple.field(1));
}
```

Resources support `Symbol.dispose` when available and throw `ClosedResourceError`
after closing. Cleanup does not depend on GC. Index and tuple-field iterators
close on exhaustion, exceptions and `for...of` early exit; explicitly close an
iterator that you stop consuming manually. A tuple cursor's `read()` keeps it
open at exhaustion for `rewind()`/`seek()`. Cleanup and rollback failures never
replace a primary exception. Tarantool failures become `TarantoolError` with the
code, message, error type, file and line supplied by WIT.

`box.transaction(() => { ... }, {isolation, synchronous})` commits on success and
rolls back on an exception. Nested transactions, async callbacks and thenable
results are rejected. Do not end or replace a transaction inside its helper.
Use a CLI `wasi:cli/run@0.2.12` export and the transaction-capable fiber mode of
`wasm.run`; `wasm.call` retains the runtime's thread/transaction restrictions.
Memtx transactions that access data require `memtx_use_mvcc_engine = true`,
because the host yields between calls. Vinyl retains its normal transaction
semantics; isolation levels follow Tarantool configuration. Export-level
ComponentizeJS async support does not make the synchronous transaction helper
safe for `await`.

ComponentizeJS executes module initialization while building. Importing this SDK
performs no host calls and creates no host resources; call database methods and
open callbacks only inside runtime exports. See the
[preinitialization documentation](https://github.com/bytecodealliance/ComponentizeJS#wizer-pre-initialization).

HTTP fetch requires enabling the compiler's HTTP feature, for example
`tarawasm build -- --enable http`; it is disabled in tarawasm's JS defaults.
The host's socket policy does not restrict WASI HTTP. Raw sockets and Node network
adapters have unresolved toolchain compatibility failures in the verified
versions; the SDK does not provide network wrappers. Built-in capability probes
record these failures separately from successful network exchanges.

## Development checks

Run `npm run test:style`, `npm test`, `npm run test:types` and `bash tests/pack.sh`
from `guests/js`. `npm run format` applies Google JavaScript formatting; public
signatures are maintained in TypeScript declarations. `tests/check-coverage.js`
checks every released WIT function against its SDK method after fetching WIT.

`tests/build-components.sh` builds, strips and validates guests importing the
installed archive with local tools. CI checks out the latest tarawasm `master`
and prepares only its JS toolchain and the Python compiler for the exchange
test. For local build checks, set `PYTHONPATH` to that checkout, prepare
`toolchain setup js` and `toolchain setup python` in local mode, and expose the
managed tool paths in `PATH`. `TWR_JS_PYTHON` can select the Python interpreter
that has tarawasm's CLI dependencies installed.

`tests/test-runtime.sh` runs the components in Tarantool-Wasm-Rs,
including the MessagePack exchange with a Python guest. The separate capability
audit uses `tests/build-capability-probes.sh` and
`tests/integration/capabilities_test.lua`. Optional external HTTPS checks use
`TWR_JS_HTTPS_PROBE_URL`; skipped or classified failures do not count as verified
network support.
