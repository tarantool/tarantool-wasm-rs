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
python3 -m pip install -q --disable-pip-version-check --target /tmp/sdk-installed /tmp/sdk-wheel/*.whl
mkdir /tmp/conformance-app
cp /workspace/guests/python/tests/components/ssl_conformance/main.py /tmp/conformance-app/main.py
cp -R /workspace/guests/python/tests/components/ssl_conformance/wit /tmp/conformance-app/wit
cp -R /workspace/guests/python/tests/conformance/upstream/cpython-3.12.11/test /tmp/conformance-app/test
python3 /workspace/guests/python/tests/conformance/generate.py --output /tmp/conformance-app/cpython_ssl_portable.py
cd /tmp/conformance-app
mkdir -p wit/deps/twr-ssl
cp /tmp/sdk-installed/twr_python_sdk/wit/ssl-bio.wit wit/deps/twr-ssl/
componentize-py -q -d wit -w ssl-conformance --features tls componentize \
    -p . -p /tmp/sdk-installed main -o /tmp/conformance.wasm
PYTHONPATH=/tmp/sdk-installed python3 /workspace/guests/python/scripts/finalize-component.py /tmp/conformance.wasm
cp /tmp/conformance.wasm /workspace/guests/python/tests/components/ssl_conformance/
'
