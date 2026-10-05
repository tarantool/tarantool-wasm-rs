# Guest SDKs

[python/](python/README.md) owns the Python SDK, ports, WIT contracts,
interpreter and tests. Other language SDKs belong in sibling directories.

Each SDK packages guest code and toolchain/host requirements. Native port
implementations are optional runtime dependencies. The core runtime owns
Wasmtime stores, resource tables and permissions; ports register through a
small runtime view. Installing a wheel does not enable a missing host feature.
