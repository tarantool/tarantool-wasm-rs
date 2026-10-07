#!/usr/bin/env bash
# Current capability audit, separate from the SDK's supported interfaces.
set -euo pipefail
task_repo_root=$(cd -- "$(dirname -- "$0")/../../.." && pwd)
task_sdk_root="$task_repo_root/guests/js"
task_python=${TWR_JS_PYTHON:-python3}
tarawasm() { "$task_python" -m tarawasm.cli --non-interactive "$@"; }
mkdir -p "$task_sdk_root/dist"
for task_mode in default http; do
    cd "$task_sdk_root/tests/components/builtins"
    task_out=fresh.wasm
    if [[ "$task_mode" == http ]]; then task_out=fresh-http.wasm; fi
    tarawasm clean
    tarawasm bind --execution local
    if [[ "$task_mode" == http ]]; then
        tarawasm build --execution local -- --enable http
    else
        tarawasm build --execution local
    fi
    tarawasm strip --execution local sdk.wasm
    mv sdk.strip.wasm sdk.wasm
    mv sdk.wasm "$task_out"
    wasm-tools validate "$task_out"
done
cd "$task_sdk_root/tests/components/sockets"
wkg fetch --type wasm
jco componentize main.js --wit wit --world-name probe -o fresh.wasm --disable all --enable stdio
wasm-tools strip fresh.wasm -o probe.tmp
mv probe.tmp fresh.wasm
wasm-tools validate fresh.wasm
# Jco adds Node adapter WIT dependencies. Work in temporary copies so the
# audit never rewrites checked-in contracts or relies on generated leftovers.
task_tmp=$(mktemp -d)
trap 'rm -rf -- "$task_tmp"' EXIT
for task_name in node-zlib node-net; do
    task_dir="$task_tmp/$task_name"
    mkdir -p "$task_dir/wit"
    cp "$task_sdk_root/tests/components/$task_name/main.js" "$task_dir/main.js"
    cp "$task_sdk_root/tests/components/$task_name/wit/world.wit" "$task_dir/wit/world.wit"
    cd "$task_dir"
    rm -f "$task_sdk_root/tests/components/$task_name/fresh.wasm"
    task_log="$task_sdk_root/dist/$task_name-build.log"
    if jco componentize main.js --bundle --wit wit --world-name probe -o fresh.wasm --disable all --enable stdio > "$task_log" 2>&1; then
        wasm-tools strip fresh.wasm -o "$task_sdk_root/tests/components/$task_name/fresh.wasm"
        wasm-tools validate "$task_sdk_root/tests/components/$task_name/fresh.wasm"
        printf 'built\n' > "$task_sdk_root/dist/$task_name-build-status.txt"
    elif [[ "$task_name" == node-net ]] && grep -q "missing function.*network-error-code" "$task_log"; then
        printf 'known-build-failure: wasi:sockets/network@0.2.10 network-error-code\n' > "$task_sdk_root/dist/node-net-build-status.txt"
    else
        cat "$task_log" >&2
        exit 1
    fi
done
