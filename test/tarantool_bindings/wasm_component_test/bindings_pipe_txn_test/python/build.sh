#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
output=${1:-"$script_dir/adder.wasm"}
case "$output" in
    /*) ;;
    *) output="$PWD/$output" ;;
esac
cd "$script_dir"
wkg wit fetch --type wasm
docker run --rm -v "$PWD:/workspace" -w /workspace mandeser0/tarawasm:latest all
docker run --rm -v "$PWD:/workspace" -w /workspace mandeser0/tarawasm:latest strip adder.wasm
mv adder.strip.wasm adder.wasm
sudo chown "$(id -u):$(id -g)" adder.wasm
wasm-tools validate adder.wasm
if [ ! adder.wasm -ef "$output" ]; then
    cp adder.wasm "$output"
fi
