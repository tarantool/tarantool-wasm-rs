#!/usr/bin/env bash
# Install the packed SDK into two unrelated application worlds outside the checkout.
set -euo pipefail

task_sdk_root=$(cd -- "$(dirname -- "$0")/.." && pwd -P)
task_repo_root=$(cd -- "$task_sdk_root/../.." && pwd -P)

: "${TWR_CSHARP_IMAGE:?Set TWR_CSHARP_IMAGE to a source-built tarawasm C# image}"

task_image=$(docker image inspect "$TWR_CSHARP_IMAGE" --format '{{.Id}}')

task_stage=$(mktemp -d)
task_stage=$(cd -- "$task_stage" && pwd -P)
trap 'rm -rf "$task_stage"' EXIT

mkdir -p "$task_sdk_root/dist/components"

for task_fixture in base alternate; do
    task_app="$task_stage/$task_fixture"
    mkdir -p "$task_app/wit"
    cp "$task_sdk_root/tests/components/$task_fixture/world.wit" "$task_app/wit/world.wit"

    run() {
        docker run --rm --pull=never --platform linux/amd64 \
            -v "$task_repo_root:/work:ro" -v "$task_stage:/stage" \
            -w "/stage/$task_fixture" "$task_image" "$@"
    }

    if [[ "$task_fixture" == base ]]; then task_world=sdk-test; else task_world=secondary; fi
    run init --lang csharp --wit wit --world "$task_world" .

    python3 - "$task_app" "$task_sdk_root/tests/components/$task_fixture/Implementation.cs" <<'PY'
import shutil, sys
from pathlib import Path

root = Path(sys.argv[1])
for source in (root / 'src').glob('*.cs'):
    source.unlink()

shutil.copy2(sys.argv[2], root / 'src/Implementation.cs')

project = root / 'Component.csproj'
project.write_text(project.read_text().replace('<!-- tarawasm invokes',
    '<PackageReference Include="Tarantool.Wasm" Version="[0.1.0]" />\n    <!-- tarawasm invokes'))

config = root / 'nuget.config'
config.write_text(config.read_text().replace('<clear />',
    '<clear /><add key="twr-sdk" value="/work/guests/csharp/dist" />'))
PY

    run dotnet restore --use-lock-file --force-evaluate -p:RestoreLockedMode=false \
        -p:RestoreFallbackFolders=/opt/tarawasm-csharp/packages --configfile nuget.config

    task_lock=$(shasum -a 256 "$task_app/packages.lock.json")
    run bind
    run build
    [[ $(shasum -a 256 "$task_app/packages.lock.json") == "$task_lock" ]] || { echo 'Lock changed during build' >&2; exit 1; }

    run strip "dist/$task_world.wasm"
    mv "$task_app/dist/$task_world.strip.wasm" "$task_app/dist/$task_world.wasm"
    run wasm-tools validate "dist/$task_world.wasm"
    run wasm-tools component wit "dist/$task_world.wasm" > "$task_sdk_root/dist/$task_fixture.component.wit"

    cp "$task_app/dist/$task_world.wasm" "$task_sdk_root/dist/components/$task_fixture.wasm"
    cp "$task_app/packages.lock.json" "$task_sdk_root/dist/$task_fixture.packages.lock.json"
done

if grep -Eq 'import (docs:adder/|wasi:sockets/|wasi:tls/)' "$task_sdk_root/dist/base.component.wit"; then
    echo 'Database SDK unexpectedly imports optional host interfaces' >&2; exit 1
fi
