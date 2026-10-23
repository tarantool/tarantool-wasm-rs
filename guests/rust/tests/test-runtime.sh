#!/usr/bin/env bash
set -euo pipefail

sdk_root=$(cd -- "$(dirname -- "$0")/.." && pwd -P)
repo_root=$(cd -- "$sdk_root/../.." && pwd -P)

run_dir=$(mktemp -d)
trap 'rm -rf "$run_dir"' EXIT
cp "$sdk_root/tests/sdk_test.lua" "$run_dir/sdk_test.lua"

export TWR_RUST_COMPONENT="$sdk_root/tests/component/target/wasm32-wasip2/release/rust_sdk_test.wasm"
export LUA_CPATH="$repo_root/.rocks/lib/tarantool/?.dylib;$repo_root/.rocks/lib/tarantool/?.so;${LUA_CPATH:-;}"

cd -- "$run_dir"
if [[ $(uname -s) == Darwin ]]; then
    timeout_cmd=gtimeout
else
    timeout_cmd=timeout
fi

"$timeout_cmd" --kill-after=10s 300s "$repo_root/.rocks/bin/luatest" -v -l luatest.log ./sdk_test.lua
