#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
python=${PYTHON:-python3}
wasm_tools=${WASM_TOOLS:-wasm-tools}
wkg=${WKG:-wkg}
output=${1:-"$script_dir/adder.wasm"}
build_dir=$(mktemp -d "${TMPDIR:-/tmp}/tarantool-python-interpreter.XXXXXX")

cleanup() {
    rm -rf -- "$build_dir"
}
trap cleanup EXIT HUP INT TERM

actual_wasm_tools_version=$($wasm_tools --version)
if [ "$actual_wasm_tools_version" != "wasm-tools 1.245.1" ]; then
    echo "expected wasm-tools 1.245.1, got: $actual_wasm_tools_version" >&2
    exit 1
fi

(
    cd "$script_dir"
    rm -rf -- "$script_dir/wit/deps"
    "$wkg" wit fetch --type wasm
)

"$python" -m pip install \
    --disable-pip-version-check \
    --target "$build_dir/site-packages" \
    -r "$script_dir/requirements-build.txt"

PYTHONPATH="$build_dir/site-packages" \
    "$python" -c 'from componentize_py import script; raise SystemExit(script())' \
    -q \
    -d "$script_dir/wit" \
    -w adder \
    --features tls \
    componentize \
    -p "$build_dir/site-packages" \
    -p "$script_dir" \
    main \
    -o "$build_dir/adder.unstripped.wasm"

"$wasm_tools" strip --all "$build_dir/adder.unstripped.wasm" -o "$output"
"$wasm_tools" validate --features component-model "$output"

echo "built $output"
