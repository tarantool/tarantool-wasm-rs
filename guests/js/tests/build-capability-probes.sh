#!/usr/bin/env bash
# Reproducible capability audit, separate from the SDK's supported interfaces.
set -euo pipefail
task_repo_root=$(cd -- "$(dirname -- "$0")/../../.." && pwd)
task_fresh=${TWR_JS_TOOLCHAIN_IMAGE:-$(node -p 'JSON.parse(require("fs").readFileSync(process.argv[1])).toolchain.tarawasm_image' "$task_repo_root/guests/js/sdk-manifest.json")}
task_legacy=mandeser0/tarawasm:v0.3.0
mkdir -p "$task_repo_root/guests/js/dist"
for task_profile in legacy fresh; do
    task_image=$task_legacy
    if [[ "$task_profile" == fresh ]]; then task_image=$task_fresh; fi
    if ! docker image inspect "$task_image" >/dev/null 2>&1; then docker pull "$task_image"; fi
    docker run --rm -v "$task_repo_root:/workspace" -e TWR_PROBE_PROFILE="$task_profile" \
        -e WKG_CONFIG_FILE=/workspace/guests/js/dist/wkg.toml \
        --entrypoint /bin/bash "$task_image" -ec '
task_root=/workspace/guests/js/tests/components
for task_mode in default http; do
    cd "$task_root/builtins"
    task_out="$TWR_PROBE_PROFILE.wasm"
    task_features=(--disable all --enable stdio)
    if [[ "$task_mode" == http ]]; then
        task_out="$TWR_PROBE_PROFILE-http.wasm"
        task_features=(--disable clocks random fetch-event --enable stdio http)
    fi
    if [[ "$TWR_PROBE_PROFILE" == fresh ]]; then
        # Test published tarawasm defaults and user feature overrides end to end.
        /app/docker-scripts/entrypoint.sh clean
        /app/docker-scripts/entrypoint.sh bind
        if [[ "$task_mode" == http ]]; then
            /app/docker-scripts/entrypoint.sh build -- --enable http
        else
            /app/docker-scripts/entrypoint.sh build
        fi
        mv sdk.wasm "$task_out"
    else
        jco componentize main.js --wit wit --world-name probe -o "$task_out" "${task_features[@]}"
    fi
    wasm-tools strip "$task_out" -o "$task_out.tmp"
    mv "$task_out.tmp" "$task_out"
    wasm-tools validate "$task_out"
done
cd "$task_root/sockets"
if [[ "$TWR_PROBE_PROFILE" == fresh ]]; then wkg fetch --type wasm; else wkg wit fetch --type wasm; fi
jco componentize main.js --wit wit --world-name probe -o "$TWR_PROBE_PROFILE.wasm" --disable all --enable stdio
wasm-tools strip "$TWR_PROBE_PROFILE.wasm" -o probe.tmp
mv probe.tmp "$TWR_PROBE_PROFILE.wasm"
wasm-tools validate "$TWR_PROBE_PROFILE.wasm"
'
done
# Jco adds Node adapter WIT dependencies. Work in temporary copies so the
# audit never rewrites checked-in contracts or relies on generated leftovers.
docker run --rm -v "$task_repo_root:/workspace" --entrypoint /bin/bash "$task_fresh" -ec '
task_root=/workspace/guests/js/tests/components
for task_name in node-zlib node-net; do
    task_tmp=$(mktemp -d)
    mkdir -p "$task_tmp/wit"
    cp "$task_root/$task_name/main.js" "$task_tmp/main.js"
    cp "$task_root/$task_name/wit/world.wit" "$task_tmp/wit/world.wit"
    cd "$task_tmp"
    rm -f "$task_root/$task_name/fresh.wasm"
    task_log="/workspace/guests/js/dist/$task_name-build.log"
    if jco componentize main.js --bundle --wit wit --world-name probe -o fresh.wasm --disable all --enable stdio > "$task_log" 2>&1; then
        wasm-tools strip fresh.wasm -o "$task_root/$task_name/fresh.wasm"
        wasm-tools validate "$task_root/$task_name/fresh.wasm"
        printf "built\n" > "/workspace/guests/js/dist/$task_name-build-status.txt"
    elif [[ "$task_name" == node-net ]] && grep -q "missing function.*network-error-code" "$task_log"; then
        printf "known-build-failure: wasi:sockets/network@0.2.10 network-error-code\n" > /workspace/guests/js/dist/node-net-build-status.txt
    else
        cat "$task_log" >&2
        exit 1
    fi
    rm -rf "$task_tmp"
done
'
