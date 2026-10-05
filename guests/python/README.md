# TWR Python SDK

`twr-python-sdk` is a wheel with a Python API for Tarantool, compatibility
ports, WIT contracts and a versioned requirements manifest. Importing it does not
replace modules; call `twr_python_sdk.install()` explicitly before importing
libraries that use SSL. The wheel contains a library API and does not depend on a compiler.

Supported componentize-py is 0.25.0 (embedded CPython 3.14.0), with wasm-tools
1.245.1. SSL targets Python 3.12; reference tests use CPython 3.12.11.
The wheel supports Python 3.10+; that does not select the embedded interpreter.

[The Tarantool API](sdk/README.md) wraps all interfaces in
`tarantool-wit@0.2.0` and the runtime's Lua callbacks. Raw bindings remain
available through `import _tarantool`. This API does not require `install()`.

## Standard library support

Importability alone does not establish full compatibility.
[SSL scope](ports/ssl/README.md) and [conformance](tests/conformance/README.md)
describe tested behavior and exclusions.

Pure Python packages can be frozen from application paths. Native C/C++ wheels
need a Wasm build or another adapter. The interpreter bundles optional HTTP
packages from `interpreter/requirements-build.txt`; the SDK does not require them.

## Build artifacts

```sh
bash guests/python/build-artifacts.sh
# Wheel only:
sh guests/python/build-wheel.sh
```

Run from the repository root. The pinned `mandeser0/tarawasm:v0.3.0` image builds
the wheel, WIT and manifest in `dist/`, plus five stripped, validated components:
interpreter, HTTPS, stdlib smoke, SSL conformance and Tarantool API checks.
Packaging dependencies and wheel timestamps are pinned. The Rust host crate
is compiled into the native module separately.

## Use with tarawasm

Build the wheel with `sh guests/python/build-wheel.sh`. Copy it into your
Python component project. With the project's existing `tarawasm.json`, use the
pinned Docker toolchain from this repository:

```sh
tarawasm() {
  docker run --rm -v "$PWD:/work" -w /work mandeser0/tarawasm:v0.3.0 "$@"
}
tarawasm pip install ./twr_python_sdk-0.1.0-py3-none-any.whl
mkdir -p wit/deps/twr-runtime
cp .tarawasm/site-packages/twr_python_sdk/wit/ssl-bio.wit wit/deps/twr-runtime/
```

Add `import docs:adder/ssl-bio@0.1.3;` to the application's WIT world.
If it already uses package `docs:adder@0.1.3`, put `ssl-bio.wit` alongside its
root WIT files and use `import ssl-bio;` instead of adding a second package.
Then begin `main.py` with:

```python
import twr_python_sdk
twr_python_sdk.install()

import ssl
import urllib.request
# Implement the application's generated WIT exports below.
```

`install()` enables SSL and the WASI asyncio DNS adapter. Select only DNS with
`install(ports=("asyncio_dns",))`; `install(ports=())` changes nothing.
Call it before importing SSL consumers. Repeated calls reuse the same adapters.
[The HTTPS fixture](tests/components/https_smoke/main.py) is a complete example.

```sh
tarawasm bind
tarawasm build
# Substitute the output name from tarawasm.json.
tarawasm wasm-tools strip --all app.wasm -o app.strip.wasm
mv app.strip.wasm app.wasm
tarawasm python3 -c 'import twr_python_sdk; twr_python_sdk.patch_socket_errors("app.wasm")'
tarawasm wasm-tools validate app.wasm
```

The pinned wasi-libc needs a separate socket-errno binary correction after
stripping. `install()` cannot change compiled libc. `patch_socket_errors()`
accepts only the known mapper body and is safe to repeat. Network access and
certificate files require the native runtime's `wasm.load` permissions; temporary
files require a writable preopen. The SSL port needs Cargo feature
`python-ssl-compat` in the native module.

## Use with componentize-py directly

Install the wheel in the build environment and add its installation directory
to componentize-py's Python search paths. Use the same WIT import and `main.py`
initialization shown above:

```sh
python3 -m pip install ./twr_python_sdk-0.1.0-py3-none-any.whl
sdk_site=$(python3 -c 'from pathlib import Path; import twr_python_sdk; print(Path(twr_python_sdk.__file__).resolve().parent.parent)')
mkdir -p wit/deps/twr-runtime
cp "$sdk_site/twr_python_sdk/wit/ssl-bio.wit" wit/deps/twr-runtime/
componentize-py -d wit -w app bindings bindings
componentize-py -d wit -w app componentize \
  -p . -p bindings -p "$sdk_site" main -o app.wasm
wasm-tools strip --all app.wasm -o app.strip.wasm
mv app.strip.wasm app.wasm
python3 -c 'import twr_python_sdk; twr_python_sdk.patch_socket_errors("app.wasm")'
wasm-tools validate app.wasm
```

Replace `app` with your world name. `twr_python_sdk.freeze_packages("your_package")`
can freeze pure Python package sources during componentization for later dynamic
imports. Native C/C++ wheels need compatible Wasm extensions.

## Host integration and removal

`ports/ssl/` owns the adapter, WIT and Rust OpenSSL host. Core supplies linker
registration and the resource-table guard through Cargo feature
`python-ssl-compat`, enabled by default. `cargo check --no-default-features`
verifies core without that dependency.

The contract source is `ports/ssl/wit/ssl-bio.wit` (`docs:adder/ssl-bio@0.1.3`),
shared by the host and wheel. Use the exact packaged contract in the application.

Drop `patch_socket_errors` after upgrading to a componentize-py release containing
the [wasi-libc socket errno fix](https://github.com/WebAssembly/wasi-libc/pull/838)
and passing socket regressions without the patch. The fix is already in
[componentize-py main](https://github.com/bytecodealliance/componentize-py/pull/236);
release 0.25.1 still needs the patch.

Drop the SSL port, its WIT import and optional Rust host when componentize-py
provides `ssl`/`_ssl` and the existing Python 3.12 conformance and BIO/socket/HTTPS
tests pass in Wasm using that implementation.

## Checks

```sh
python3 -m pip install msgpack==1.0.3
python3 -m unittest discover -s guests/python/tests/unit -p '*_test.py'
cargo test --locked -p twr-python-ssl-host --lib
bash guests/python/test.sh
```

[guests-python.yml](../../.github/workflows/guests-python.yml) builds installed-wheel
consumers and runs native/guest checks on macOS and Linux, preserving conformance
JSON reports. Generic interpreter API tests stay in `test/lualib`.
