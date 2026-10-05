#!/usr/bin/env bash
set -euo pipefail
task_sdk_root=$(cd -- "$(dirname -- "$0")" && pwd)
task_repo_root=$(cd -- "$task_sdk_root/../.." && pwd)
cd "$task_repo_root"
export PATH="$task_repo_root/.rocks/bin:$PATH"
task_result=0
for task_test in guests/python/tests/integration/*_test.lua; do
    if [[ $(uname -s) == Darwin ]]; then
        if ! gtimeout --kill-after=10s 300s luatest -v -l "${task_test##*/}.log" "$task_test"; then
            task_result=1
        fi
    elif ! timeout --kill-after=10s 300s luatest -v -l "${task_test##*/}.log" "$task_test"; then
        task_result=1
    fi
done
exit "$task_result"
