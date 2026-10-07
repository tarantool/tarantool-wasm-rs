#!/usr/bin/env bash
# Test fixture preparation only. Applications use tarawasm's existing commands.
set -euo pipefail
task_repo_root=$(cd -- "$(dirname -- "$0")/../../.." && pwd)
task_sdk_root="$task_repo_root/guests/js"
task_image=${TWR_JS_TOOLCHAIN_IMAGE:-$(node -p 'JSON.parse(require("fs").readFileSync(process.argv[1])).toolchain.tarawasm_image' "$task_sdk_root/sdk-manifest.json")}
if ! docker image inspect "$task_image" >/dev/null 2>&1; then
    docker pull "$task_image"
fi
mkdir -p "$task_sdk_root/dist"
cd "$task_sdk_root"
npm pack --pack-destination dist
# Prepare fixtures from the actual archive. No SDK source imports in JS guests.
docker run --rm -v "$task_repo_root:/workspace" \
    -e WKG_CONFIG_FILE=/workspace/guests/js/dist/wkg.toml \
    --entrypoint /bin/bash "$task_image" -ec '
task_root=/workspace/guests/js
bash /workspace/scripts/add_tarantool_registry.sh
for task_fixture in base extras; do
    task_dir="$task_root/tests/components/$task_fixture"
    if [[ "$task_fixture" == extras ]]; then
        mkdir -p "$task_dir/wit/deps/adder"
        cp "$task_root/wit/runtime/host.wit" "$task_dir/wit/deps/adder/host.wit"
    fi
    cd "$task_dir"
    npm install --ignore-scripts --no-audit --no-fund --no-package-lock --no-save "$task_root/dist/twr-js-sdk-0.1.0.tgz"
    node --input-type=module -e '\''
        const resolved = import.meta.resolve("twr-js-sdk");
        const root = new URL("./node_modules/twr-js-sdk/", import.meta.url);
        if (!resolved.startsWith(root.href)) {
            throw new Error("Fixture must import the installed archive: " + resolved);
        }
    '\''
    if [[ "$task_fixture" != extras ]]; then wkg fetch --type wasm; fi
done
sh /workspace/guests/python/build-wheel.sh /workspace/guests/python/dist
cd "$task_root/tests/components/python-peer"
wkg fetch --type wasm
'
for task_fixture in base extras python-peer; do
    task_rel="guests/js/tests/components/$task_fixture"
    task_args=(--rm -v "$task_repo_root:/workspace" -w "/workspace/$task_rel"
        -e WKG_CONFIG_FILE=/workspace/guests/js/dist/wkg.toml)
    docker run "${task_args[@]}" "$task_image" clean
    docker run "${task_args[@]}" "$task_image" bind
    if [[ "$task_fixture" == python-peer ]]; then
        # Reinstall into the persistent mounted fixture after clean/bind.
        docker run "${task_args[@]}" --entrypoint /bin/bash "$task_image" -ec '
            python3 -m pip install --upgrade --disable-pip-version-check --target .tarawasm/sdk-installed /workspace/guests/python/dist/twr_python_sdk-0.1.0-py3-none-any.whl'
        docker run "${task_args[@]}" "$task_image" build -- --python-path .tarawasm/sdk-installed
    else
        # Exercise tarawasm's default npm dependency bundling.
        docker run "${task_args[@]}" "$task_image" build
    fi
    docker run "${task_args[@]}" "$task_image" strip sdk.wasm
    mv "$task_repo_root/$task_rel/sdk.strip.wasm" "$task_repo_root/$task_rel/sdk.wasm"
    docker run "${task_args[@]}" --entrypoint wasm-tools "$task_image" validate sdk.wasm
    docker run "${task_args[@]}" --entrypoint wasm-tools "$task_image" component wit sdk.wasm > "$task_sdk_root/dist/$task_fixture.component.wit"
done
if grep -Eq "docs:adder/|wasi:tls" "$task_sdk_root/dist/base.component.wit"; then
    echo "Base SDK component imports an optional runtime interface" >&2; exit 1
fi
docker run --rm -v "$task_repo_root:/workspace" --entrypoint /bin/bash "$task_image" -ec '
node /workspace/guests/js/tests/observe-toolchain.js
'
