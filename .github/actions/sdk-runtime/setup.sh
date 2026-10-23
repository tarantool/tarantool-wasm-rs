#!/usr/bin/env bash
set -euo pipefail

sudo sh scripts/install_openssl_dependencies.sh
curl --fail --location --retry 3 https://tarantool.io/release/3/installer.sh | sudo bash
sudo apt-get install -y 'tarantool=3.7.*' 'tarantool-dev=3.7.*'
curl --fail --location --retry 3 https://tarantool.io/release/2/installer.sh | sudo bash
sudo apt-get install -y tt

tt rocks install luatest
export PATH="$PWD/.rocks/bin:$PATH"

test -s .rocks/lib/tarantool/wasm.so
LUA_CPATH="$PWD/.rocks/lib/tarantool/?.so;;" tarantool -e "require('wasm')" </dev/null
