#!/bin/sh
set -eu
script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
sdk_dir=$(CDPATH='' cd -- "$script_dir/.." && pwd)
python=${PYTHON:-python3}
output=${1:-"$script_dir/adder.wasm"}
build_dir=$(mktemp -d "${TMPDIR:-/tmp}/twr-python-interpreter.XXXXXX")
trap 'rm -rf -- "$build_dir"' EXIT HUP INT TERM

"$python" -m pip install --disable-pip-version-check --target "$build_dir/site-packages" \
    "$sdk_dir/dist/twr_python_sdk-0.1.0-py3-none-any.whl" -r "$script_dir/requirements-build.txt"
mkdir "$build_dir/wit"
cp "$script_dir"/wit/*.wit "$build_dir/wit/"
cp "$script_dir/wkg.lock" "$build_dir/"
cp "$build_dir/site-packages/twr_python_sdk/wit/ssl-bio.wit" "$build_dir/wit/"
cp "$build_dir/site-packages/twr_python_sdk/wit/lua-callback.wit" "$build_dir/wit/"
sed "s#../../../third-party/#$sdk_dir/../../third-party/#" \
    "$script_dir/wkg.toml" > "$build_dir/wkg.toml"
(cd "$build_dir" && "${WKG:-wkg}" wit fetch --type wasm)
"$python" "$sdk_dir/scripts/prepare-wit.py" "$build_dir/wit" "$build_dir/resolved-wit" adder
"$python" -c 'from componentize_py import script; raise SystemExit(script())' \
    -q -d "$build_dir/resolved-wit" -w adder --features tls componentize \
    -p "$script_dir" -p "$build_dir/site-packages" main -o "$output"
PYTHONPATH="$build_dir/site-packages" "$python" \
    "$sdk_dir/scripts/finalize-component.py" "$output"
