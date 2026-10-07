#!/usr/bin/env bash
set -euo pipefail
task_sdk_root=$(cd -- "$(dirname -- "$0")" && pwd)
task_repo_root=$(cd -- "$task_sdk_root/../.." && pwd)
task_image=mandeser0/tarawasm:v0.3.0
if ! docker image inspect "$task_image" >/dev/null 2>&1; then
    docker pull "$task_image"
fi
docker run --rm -v "$task_repo_root:/workspace" \
    -e WKG_CONFIG_FILE=/tmp/sdk-wkg-config.toml \
    --entrypoint /bin/bash "$task_image" -ec '
bash /workspace/scripts/add_tarantool_registry.sh
sh /workspace/guests/python/build-wheel.sh /workspace/guests/python/dist
sh /workspace/guests/python/interpreter/build.sh
mkdir -p /workspace/guests/python/dist/wit
cp /workspace/guests/python/ports/ssl/wit/ssl-bio.wit /workspace/guests/python/dist/wit/
cp /workspace/guests/python/sdk/src/twr_python_sdk/wit/lua-callback.wit /workspace/guests/python/dist/wit/
cp /workspace/guests/python/sdk-manifest.json /workspace/guests/python/dist/
python3 -m pip install --disable-pip-version-check --target /tmp/sdk-installed \
    /workspace/guests/python/dist/twr_python_sdk-0.1.0-py3-none-any.whl
mkdir /tmp/stdlib-app
cp /workspace/guests/python/tests/components/stdlib_smoke/main.py /tmp/stdlib-app/main.py
cp -R /workspace/guests/python/tests/components/stdlib_smoke/wit /tmp/stdlib-app/wit
cd /tmp/stdlib-app
componentize-py -q -d wit -w stdlib-smoke --features tls componentize \
    -p . -p /tmp/sdk-installed main -o /tmp/stdlib.wasm
PYTHONPATH=/tmp/sdk-installed python3 /workspace/guests/python/scripts/finalize-component.py /tmp/stdlib.wasm
wasm-tools component wit /tmp/stdlib.wasm > /tmp/stdlib.wit
if grep -q "docs:adder/ssl-bio" /tmp/stdlib.wit; then
    echo "stdlib component unexpectedly imports the SDK SSL interface" >&2
    exit 1
fi
cp /tmp/stdlib.wasm /workspace/guests/python/tests/components/stdlib_smoke/
mkdir /tmp/tarantool-app
cp /workspace/guests/python/tests/components/tarantool/main.py /tmp/tarantool-app/main.py
cp -R /workspace/guests/python/tests/components/tarantool/wit /tmp/tarantool-app/wit
cp /tmp/sdk-installed/twr_python_sdk/wit/lua-callback.wit /tmp/tarantool-app/wit/
cd /tmp/tarantool-app
wkg wit fetch --wit-dir wit --type wasm
python3 /workspace/guests/python/scripts/prepare-wit.py wit /tmp/tarantool-wit sdk-test
componentize-py -q -d /tmp/tarantool-wit -w sdk-test componentize \
    -p . -p /tmp/sdk-installed main -o /tmp/tarantool.wasm
PYTHONPATH=/tmp/sdk-installed python3 /workspace/guests/python/scripts/finalize-component.py /tmp/tarantool.wasm
cp /tmp/tarantool.wasm /workspace/guests/python/tests/components/tarantool/
'
bash "$task_sdk_root/tests/components/https_smoke/build.sh"
bash "$task_sdk_root/tests/components/ssl_conformance/build.sh"
