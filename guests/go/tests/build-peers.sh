#!/usr/bin/env bash
# Only this explicitly cross-language check needs Python/JS guest toolchains.
set -euo pipefail
task_repo_root=$(cd -- "$(dirname -- "$0")/../../.." && pwd)
task_python=${TWR_GO_PYTHON:-python3}
tarawasm() { "$task_python" -m tarawasm.cli --non-interactive "$@"; }
PYTHON="$task_python" sh "$task_repo_root/guests/python/build-wheel.sh" "$task_repo_root/guests/python/dist"
(cd "$task_repo_root/guests/js" && npm ci --ignore-scripts && mkdir -p dist && npm pack --pack-destination dist)
for task_language in python js; do
 task_dir="$task_repo_root/guests/go/tests/components/$task_language-peer"
 cd "$task_dir"
 wkg fetch --type wasm
 tarawasm clean
 tarawasm bind --execution local
 if [[ "$task_language" == python ]]; then
  "$task_python" -m pip install --upgrade --target .tarawasm/sdk-installed \
   "$task_repo_root/guests/python/dist/twr_python_sdk-0.1.0-py3-none-any.whl"
  tarawasm build --execution local -- --python-path .tarawasm/sdk-installed
 else
  npm install --ignore-scripts --no-audit --no-fund --no-package-lock --no-save \
   "$task_repo_root/guests/js/dist/twr-js-sdk-0.1.0.tgz"
  tarawasm build --execution local
 fi
 tarawasm strip --execution local sdk.wasm
 mv sdk.strip.wasm sdk.wasm
 wasm-tools validate sdk.wasm
done
