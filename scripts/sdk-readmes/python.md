# Python SDK

Package: `twr-python-sdk` wheel. Matching WIT: `../../wit/python/`.

Dependencies:

- Python 3.10+ and pip to install the wheel.
- `msgpack==1.0.3`; the SDK uses its pure Python fallback.
- componentize-py 0.25.1 (embedded CPython 3.14.0).
- wasm-tools 1.261.0 for component stripping and validation.
- `tarantool:tarantool@0.2.0`, `docs:adder/lua-callback@0.1.3` and optional
  `docs:adder/ssl-bio@0.1.3`; the supplied WIT also includes WASI CLI 0.2.12.

Installation and build instructions:
[tarawasm](https://github.com/mandesero/tarawasm).
