#!/usr/bin/env bash
# Build consumers outside the checkout, using only the installed SDK wheel.
set -euo pipefail
sdk_root=$(cd -- "$(dirname -- "$0")/.." && pwd)
python=${PYTHON:-python3}
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT

bash "$sdk_root/../../scripts/add_tarantool_registry.sh"
"$python" -m pip install --disable-pip-version-check --target "$stage/site-packages" \
    "$sdk_root/dist/twr_python_sdk-0.1.0-py3-none-any.whl"
PYTHON="$python" sh "$sdk_root/interpreter/build.sh"

for fixture in stdlib_smoke tarantool https_smoke ssl_conformance; do
    app="$stage/$fixture"
    mkdir -p "$app/wit"
    cp "$sdk_root/tests/components/$fixture/main.py" "$app/"
    cp "$sdk_root/tests/components/$fixture"/wit/*.wit "$app/wit/"
    wit="$app/wit"
    features=()

    case "$fixture" in
        stdlib_smoke)
            world=stdlib-smoke
            output=stdlib.wasm
            features=(--features tls)
            ;;
        tarantool)
            world=sdk-test
            output=tarantool.wasm
            cp "$stage/site-packages/twr_python_sdk/wit/lua-callback.wit" "$wit/"
            cp "$sdk_root/wkg.toml" "$sdk_root/wkg.lock" "$app/"
            (cd "$app" && "${WKG:-wkg}" wit fetch --type wasm)
            "$python" "$sdk_root/scripts/prepare-wit.py" "$wit" "$app/resolved-wit" "$world"
            wit="$app/resolved-wit"
            ;;
        https_smoke|ssl_conformance)
            mkdir -p "$wit/deps/twr-ssl"
            cp "$stage/site-packages/twr_python_sdk/wit/ssl-bio.wit" "$wit/deps/twr-ssl/"
            if [[ "$fixture" == https_smoke ]]; then
                world=ssl-client
                output=client.wasm
            else
                world=ssl-conformance
                output=conformance.wasm
                features=(--features tls)
                cp -R "$sdk_root/tests/conformance/upstream/cpython-3.12.11/test" "$app/"
                "$python" "$sdk_root/tests/conformance/generate.py" --output "$app/cpython_ssl_portable.py"
            fi
            ;;
    esac

    componentize-py -q -d "$wit" -w "$world" "${features[@]}" componentize \
        -p "$app" -p "$stage/site-packages" main -o "$app/$output"
    PYTHONPATH="$stage/site-packages" "$python" \
        "$sdk_root/scripts/finalize-component.py" "$app/$output"
    if [[ "$fixture" == stdlib_smoke ]]; then
        wasm-tools component wit "$app/$output" > "$app/component.wit"
        if grep -q 'docs:adder/ssl-bio' "$app/component.wit"; then
            echo 'stdlib component unexpectedly imports the SDK SSL interface' >&2
            exit 1
        fi
    fi
    cp "$app/$output" "$sdk_root/tests/components/$fixture/"
done
