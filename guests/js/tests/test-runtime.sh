#!/usr/bin/env bash
set -euo pipefail
task_repo_root=$(cd -- "$(dirname -- "$0")/../../.." && pwd)
cd "$task_repo_root"
export PATH="$task_repo_root/.rocks/bin:$PATH"
if [[ $(uname -s) == Darwin ]]; then
    gtimeout --kill-after=10s 300s luatest -v guests/js/tests/integration/sdk_test.lua
else
    timeout --kill-after=10s 300s luatest -v guests/js/tests/integration/sdk_test.lua
fi
