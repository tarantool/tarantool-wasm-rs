#!/usr/bin/env bash
# Build Tarantool with WASM module support.
set -euo pipefail

FORK_URL="https://github.com/mandesero/tarantool"
BRANCH="wasm-module"

cd third-party/tarantool

if ! git remote get-url upstream &>/dev/null; then
    git remote add upstream "$FORK_URL"
fi

current_branch="$(git symbolic-ref --short HEAD || true)"

if [[ "$current_branch" != "$BRANCH" ]]; then
    git fetch upstream "$BRANCH"
    git checkout -B "$BRANCH" "upstream/$BRANCH"
fi

cmake -S . -B "build" \
    -DCMAKE_BUILD_TYPE=Debug \
    -DENABLE_BACKTRACE=ON \
    -DTARANTOOL_WASM=ON

cmake --build "build" -j"$(nproc)"
