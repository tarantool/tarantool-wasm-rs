#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
python=${PYTHON:-python3}
wasm_tools=${WASM_TOOLS:-wasm-tools}
output=${1:-"$script_dir/adder.wasm"}
build_dir=$(mktemp -d "${TMPDIR:-/tmp}/tarantool-pipe-txn.XXXXXX")

cleanup() {
    rm -rf -- "$build_dir"
}
trap cleanup EXIT HUP INT TERM

"$python" -m pip install \
    --disable-pip-version-check \
    --target "$build_dir/site-packages" \
    -r "$script_dir/requirements-build.txt"

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
