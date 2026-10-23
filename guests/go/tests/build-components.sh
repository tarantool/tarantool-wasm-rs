#!/usr/bin/env bash
# Fixture build/installation verification, using tarawasm's existing commands.
set -euo pipefail
task_repo_root=$(cd -- "$(dirname -- "$0")/../../.." && pwd)
task_sdk_root="$task_repo_root/guests/go"
task_python=${TWR_GO_PYTHON:-python3}
tarawasm() { "$task_python" -m tarawasm.cli --non-interactive "$@"; }
task_stage=$(mktemp -d)
task_stage=$(cd "$task_stage" && pwd -P)
trap 'rm -rf "$task_stage"' EXIT
mkdir -p "$task_sdk_root/dist"
tar -xzf "$task_sdk_root/dist/twr-go-sdk-0.1.0.tar.gz" -C "$task_stage"
task_installed="$task_stage/twr-go-sdk-0.1.0"
bash "$task_repo_root/scripts/add_tarantool_registry.sh"
for task_fixture in base extras probes; do
 task_template="$task_sdk_root/tests/components/$task_fixture"
 task_dir="$task_stage/$task_fixture"
 mkdir -p "$task_dir/wit"
 cp "$task_template"/{main.go,go.mod,go.sum,tarawasm.json,wkg.toml} "$task_dir/"
 cp "$task_template/wit/world.wit" "$task_dir/wit/"
 if [[ -f "$task_template/wkg.lock" ]]; then cp "$task_template/wkg.lock" "$task_dir/"; fi
 cd -P "$task_dir"
 go mod edit -replace="github.com/tarantool/tarantool-wasm-rs/guests/go=$task_installed"
 if [[ "$task_fixture" == extras ]]; then
  mkdir -p runtime-wit
  cp "$task_installed/wit/runtime/host.wit" runtime-wit/host.wit
  printf '[overrides]\n"docs:adder" = { path = "runtime-wit" }\n' > wkg.toml
 fi
 wkg fetch --type wasm
 tarawasm bind --execution local -- --versioned
 if [[ "$task_fixture" == probes ]]; then
  # Capability audit only: asyncify remains necessary for net/http's internal goroutines.
  tarawasm build --execution local
 else
  tarawasm build --execution local -- -scheduler=none -panic-unwind=explicit
 fi
 tarawasm strip --execution local sdk.wasm
 mv sdk.strip.wasm sdk.wasm
 wasm-tools validate sdk.wasm
 cp sdk.wasm "$task_template/"
 cp wkg.lock go.sum "$task_template/"
 wasm-tools component wit sdk.wasm > "$task_sdk_root/dist/$task_fixture.component.wit"
 if [[ "$task_fixture" == base || "$task_fixture" == probes ]]; then
  go tool wit-bindgen-go generate --versioned --world sdk-test -o internal wit
  if [[ "$task_fixture" == base ]]; then
   tinygo build -target=wasip2 -scheduler=none -panic-unwind=explicit \
    --wit-package wit --wit-world sdk-test -o direct.wasm .
  else
   tinygo build -target=wasip2 --wit-package wit --wit-world sdk-test -o direct.wasm .
  fi
  wasm-tools strip direct.wasm -o direct.wasm
  wasm-tools validate direct.wasm
  cp direct.wasm "$task_template/"
 fi
done
if grep -Eq 'import (docs:adder/|wasi:tls|wasi:sockets/)' "$task_sdk_root/dist/base.component.wit"; then
 echo 'Base SDK imports optional host interfaces' >&2; exit 1
fi
for task_import in instance-network ip-name-lookup tcp tcp-create-socket; do
 if ! grep -q "import wasi:sockets/$task_import@0.2.12" "$task_sdk_root/dist/probes.component.wit"; then
  echo "Network fixture is missing $task_import" >&2; exit 1
 fi
done
if grep -q 'network-error-code' "$task_sdk_root/dist/probes.component.wit"; then
 echo 'Network fixture imports an unstable WASI function' >&2; exit 1
fi
if ! grep -q 'docs:adder/lua-callback@0.1.3' "$task_sdk_root/dist/extras.component.wit" || \
   ! grep -q 'docs:adder/zlib@0.1.3' "$task_sdk_root/dist/extras.component.wit"; then
 echo 'Optional fixture is missing required imports' >&2; exit 1
fi
{
 go version
 tinygo version
 go tool wit-bindgen-go --version
 wasm-tools --version
 wkg --version
 wasm-opt --version
 printf '%s\n' 'tarawasm: latest (native managed Go toolchain)'
} > "$task_sdk_root/dist/toolchain-observed.txt"
