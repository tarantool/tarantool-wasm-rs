#!/usr/bin/env bash
set -euo pipefail
task_fixture_dir=$(cd -- "$(dirname -- "$0")" && pwd)
task_repo_root=$(cd -- "$task_fixture_dir/../../../../.." && pwd)
task_image=mandeser0/tarawasm:v0.3.0
if ! docker image inspect "$task_image" >/dev/null 2>&1; then
    docker pull "$task_image"
fi
docker run --rm -v "$task_repo_root:/workspace" \
    --entrypoint /bin/sh "$task_image" -ec '
sh /workspace/guests/python/build-wheel.sh /tmp/sdk-wheel
export TARAWASM_PY_SITE_PACKAGES=/tmp/sdk-installed
tarawasm() { /app/docker-scripts/entrypoint.sh "$@"; }
tarawasm pip install -q --disable-pip-version-check /tmp/sdk-wheel/*.whl
# Build outside the checkout with only the installed wheel and application inputs.
mkdir /tmp/https-app
cp /workspace/guests/python/tests/components/https_smoke/main.py /tmp/https-app/main.py
cp -R /workspace/guests/python/tests/components/https_smoke/wit /tmp/https-app/wit
cp /workspace/guests/python/tests/components/https_smoke/tarawasm.json /tmp/https-app/
cd /tmp/https-app
mkdir -p wit/deps/twr-ssl
cp /tmp/sdk-installed/twr_python_sdk/wit/ssl-bio.wit wit/deps/twr-ssl/
tarawasm bind
tarawasm build
PYTHONPATH=/tmp/sdk-installed python3 /workspace/guests/python/scripts/finalize-component.py \
    client.wasm
cp client.wasm /workspace/guests/python/tests/components/https_smoke/
'
