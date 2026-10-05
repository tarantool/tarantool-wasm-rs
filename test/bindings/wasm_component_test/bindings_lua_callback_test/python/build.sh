#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_dir=$(CDPATH= cd -- "$script_dir/../../../../.." && pwd)
python=${PYTHON:-python3}
wasm_tools=${WASM_TOOLS:-wasm-tools}
output=${1:-"$script_dir/adder.wasm"}
build_dir=$(mktemp -d "${TMPDIR:-/tmp}/tarantool-lua-handler.XXXXXX")

cleanup() {
    rm -rf -- "$build_dir"
}
trap cleanup EXIT HUP INT TERM

actual_wasm_tools_version=$($wasm_tools --version)
if [ "$actual_wasm_tools_version" != "wasm-tools 1.245.1" ]; then
    echo "expected wasm-tools 1.245.1, got: $actual_wasm_tools_version" >&2
    exit 1
fi

"$python" -m pip install \
    --disable-pip-version-check \
    --target "$build_dir/site-packages" \
    -r "$repo_dir/guests/python/interpreter/requirements-build.txt"

PYTHONPATH="$build_dir/site-packages" \
    "$python" -c 'from componentize_py import script; raise SystemExit(script())' \
    -q \
    -d "$script_dir/wit/adder.wit" \
    -w adder \
    componentize \
    -p "$build_dir/site-packages" \
    -p "$script_dir" \
    main \
    -o "$build_dir/adder.unstripped.wasm"

"$wasm_tools" strip --all "$build_dir/adder.unstripped.wasm" -o "$output"
"$wasm_tools" validate --features component-model "$output"
echo "built $output"
