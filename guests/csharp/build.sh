#!/usr/bin/env bash
set -euo pipefail

task_sdk_root=$(cd -- "$(dirname -- "$0")" && pwd -P)
task_repo_root=$(cd -- "$task_sdk_root/../.." && pwd -P)

: "${TWR_CSHARP_IMAGE:?Set TWR_CSHARP_IMAGE to a source-built tarawasm C# image (Linux amd64)}"

task_image=$(docker image inspect "$TWR_CSHARP_IMAGE" --format '{{.Id}}')
task_platform=$(docker image inspect "$task_image" --format '{{.Os}}/{{.Architecture}}')
[[ "$task_platform" == linux/amd64 ]] || { echo "C# requires a Linux amd64 image" >&2; exit 1; }

mkdir -p "$task_sdk_root/dist" "$task_sdk_root/wit/deps"

python3 - "$task_sdk_root" <<'PY_CHECK'
import hashlib, json, sys
from pathlib import Path

root = Path(sys.argv[1])
manifest = json.loads((root / 'sdk-manifest.json').read_text())
actual = hashlib.sha256((root / 'wit/contracts/tarantool.wit').read_bytes()).hexdigest()

if actual != manifest['api']['contract_sha256']:
    raise SystemExit('Bundled WIT contract does not match sdk-manifest.json')
PY_CHECK

cp "$task_sdk_root/wit/contracts/tarantool.wit" "$task_sdk_root/wit/deps/tarantool.wit"

run() {
    docker run --rm --pull=never --platform linux/amd64 -v "$task_repo_root:/work" \
        -w /work/guests/csharp "$task_image" "$@"
}

run doctor --lang csharp --json > "$task_sdk_root/dist/toolchain-observed.json"

python3 - "$task_sdk_root" <<'PY_CHECK'
import json, sys
from pathlib import Path

root = Path(sys.argv[1])
expected = json.loads((root / 'sdk-manifest.json').read_text())['toolchain']
reports = json.loads((root / 'dist/toolchain-observed.json').read_text())['reports']
report = next(item for item in reports if item['language'] == 'csharp')

if not report['ready']:
    raise SystemExit('C# toolchain is not ready')

dotnet = next(item for item in report['tools'] if item['tool'] == 'dotnet')
if dotnet['version'] != expected['dotnet']:
    raise SystemExit('Unexpected .NET SDK version')

for observed, configured in [('sdk', 'componentize_dotnet'), ('llvm', 'nativeaot_llvm'),
                              ('wit_bindgen', 'wit_bindgen'), ('wasi_sdk', 'wasi_sdk')]:
    if dotnet['components'][observed] != expected[configured]:
        raise SystemExit('Unexpected C# toolchain component: ' + observed)
PY_CHECK

task_bindgen=/opt/tarawasm-csharp/packages/bytecodealliance.componentize.dotnet.witbindgen/0.8.0-preview00011/tools/linux-x64/wit-bindgen
[[ $(run dotnet --version) == 10.0.402 ]] || { echo "Unexpected .NET SDK" >&2; exit 1; }
run "$task_bindgen" --version
run "$task_bindgen" c-sharp wit --world tarantool-sdk --runtime native-aot \
    --with-wit-results --internal --out-dir src/obj/bindings

run dotnet run --project tests/unit/Unit.csproj -c Release
run dotnet pack src/Tarantool.Wasm.csproj -c Release -o dist
printf '%s\n' "$task_image" > "$task_sdk_root/dist/image-id.txt"
