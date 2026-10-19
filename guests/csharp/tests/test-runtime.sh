#!/usr/bin/env bash
set -euo pipefail

task_sdk_root=$(cd -- "$(dirname -- "$0")/.." && pwd -P)
task_repo_root=$(cd -- "$task_sdk_root/../.." && pwd -P)

task_run_dir=$(mktemp -d)
trap 'rm -rf "$task_run_dir"' EXIT
cp "$task_sdk_root/tests/integration/sdk_test.lua" "$task_run_dir/sdk_test.lua"

export TWR_CSHARP_COMPONENTS="$task_sdk_root/dist/components"
export LUA_CPATH="$task_repo_root/.rocks/lib/tarantool/?.dylib;$task_repo_root/.rocks/lib/tarantool/?.so;${LUA_CPATH:-;}"

cd -- "$task_run_dir"
if [[ $(uname -s) == Darwin ]]; then task_timeout=gtimeout; else task_timeout=timeout; fi

"$task_timeout" --kill-after=10s 300s "$task_repo_root/.rocks/bin/luatest" -v -l luatest.log ./sdk_test.lua
