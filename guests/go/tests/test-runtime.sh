#!/usr/bin/env bash
set -euo pipefail
task_repo_root=$(cd -- "$(dirname -- "$0")/../../.." && pwd)
cd "$task_repo_root"
export PATH="$task_repo_root/.rocks/bin:$PATH"
if [[ $(uname -s) == Darwin ]]; then task_timeout=gtimeout; else task_timeout=timeout; fi
# Preserve luatest's exit code and always retain luatest.log for CI artifacts.
"$task_timeout" --kill-after=10s 300s luatest -v -l luatest.log guests/go/tests/integration/sdk_test.lua
