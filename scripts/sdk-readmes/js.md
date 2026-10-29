# JavaScript SDK

Package: `twr-js-sdk` npm archive. Matching WIT: `../../wit/js/`.

Dependencies:

- Node.js 24+ and npm.
- Jco 1.37.0, ComponentizeJS 0.23.0 and preview2-shim 0.17.8.
- wasm-tools 1.261.0 for component stripping and validation.
- `tarantool:tarantool@0.2.0` and optional
  `docs:adder/lua-callback@0.1.3` / `docs:adder/zlib@0.1.3` (supplied WIT).

The SDK has no external runtime npm dependencies. TypeScript declarations are
included; TypeScript is only needed when compiling your own TypeScript sources.

Installation and build instructions:
[tarawasm](https://github.com/mandesero/tarawasm).
