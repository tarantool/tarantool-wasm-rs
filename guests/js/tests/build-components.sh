#!/usr/bin/env bash
# Test fixture preparation only. Applications use tarawasm's existing commands.
set -euo pipefail
task_repo_root=$(cd -- "$(dirname -- "$0")/../../.." && pwd)
task_sdk_root="$task_repo_root/guests/js"
task_python=${TWR_JS_PYTHON:-python3}
tarawasm() { "$task_python" -m tarawasm.cli --non-interactive "$@"; }
bash "$task_repo_root/scripts/add_tarantool_registry.sh"
# Prepare fixtures from the actual archive. No SDK source imports in JS guests.
for task_fixture in base extras; do
    task_dir="$task_sdk_root/tests/components/$task_fixture"
    if [[ "$task_fixture" == extras ]]; then
        mkdir -p "$task_dir/wit/deps/adder"
        cp "$task_sdk_root/wit/runtime/host.wit" "$task_dir/wit/deps/adder/host.wit"
    fi
    cd "$task_dir"
    npm install --ignore-scripts --no-audit --no-fund --no-package-lock --no-save "$task_sdk_root/dist/twr-js-sdk-0.1.0.tgz"
    node --input-type=module -e '
        const resolved = import.meta.resolve("twr-js-sdk");
        const root = new URL("./node_modules/twr-js-sdk/", import.meta.url);
        if (!resolved.startsWith(root.href)) {
            throw new Error("Fixture must import the installed archive: " + resolved);
        }
    '
    if [[ "$task_fixture" != extras ]]; then wkg fetch --type wasm; fi
done
PYTHON="$task_python" just --justfile "$task_repo_root/justfile" python-sdk
cd "$task_sdk_root/tests/components/python-peer"
wkg fetch --type wasm
for task_fixture in base extras python-peer; do
    cd "$task_sdk_root/tests/components/$task_fixture"
    tarawasm clean
    tarawasm bind --execution local
    if [[ "$task_fixture" == python-peer ]]; then
        # Reinstall after clean/bind, then pass the installed SDK to the compiler.
        "$task_python" -m pip install --upgrade --disable-pip-version-check --target .tarawasm/sdk-installed "$task_repo_root/guests/python/dist/twr_python_sdk-0.1.0-py3-none-any.whl"
        tarawasm build --execution local -- --python-path .tarawasm/sdk-installed
    else
        # Exercise tarawasm's default npm dependency bundling.
        tarawasm build --execution local
    fi
    tarawasm strip --execution local sdk.wasm
    mv sdk.strip.wasm sdk.wasm
    wasm-tools validate sdk.wasm
    wasm-tools component wit sdk.wasm > "$task_sdk_root/dist/$task_fixture.component.wit"
done
if grep -Eq "docs:adder/|wasi:tls" "$task_sdk_root/dist/base.component.wit"; then
    echo "Base SDK component imports an optional runtime interface" >&2; exit 1
fi
