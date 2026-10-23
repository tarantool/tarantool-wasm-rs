#!/usr/bin/env bash
# Check installation of the archive built by just js-sdk.
set -euo pipefail

task_root=$(cd -- "$(dirname -- "$0")/.." && pwd)
task_app=$(mktemp -d)
trap 'rm -rf "$task_app"' EXIT

cd "$task_app"
npm init -y >/dev/null
npm pkg set type=module >/dev/null
npm install --ignore-scripts --no-audit --no-fund "$task_root/dist/twr-js-sdk-0.1.0.tgz"

cp "$task_root/tests/pack-consumer.js" "$task_app/consumer.js"
node --import "$task_root/tests/unit/wit-loader.js" consumer.js

mkdir types
cp "$task_root/tests/types/api.ts" types/
cp "$task_root/tests/types/tsconfig.json" types/
"$task_root/node_modules/.bin/tsc" -p types/tsconfig.json
