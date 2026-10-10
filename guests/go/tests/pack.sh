#!/usr/bin/env bash
# Build the local module archive and test installation outside the checkout.
set -euo pipefail
task_sdk_root=$(cd -- "$(dirname -- "$0")/.." && pwd)
cd "$task_sdk_root"
mkdir -p dist
task_version=0.1.0
task_stage=$(mktemp -d)
trap 'rm -rf "$task_stage"' EXIT
task_module="$task_stage/twr-go-sdk-$task_version"
mkdir -p "$task_module/wit"
cp ./*.go go.mod go.sum README.md sdk-manifest.json "$task_module/"
cp -R bindings codec luacallback raw "$task_module/"
cp wit/sdk.wit "$task_module/wit/"
cp -R wit/runtime wit/contracts "$task_module/wit/"
tar -czf "dist/twr-go-sdk-$task_version.tar.gz" -C "$task_stage" "twr-go-sdk-$task_version"
# Installation has no source checkout or application internal dependency.
for task_name in first.example/app second.example/different; do
 task_project=$(mktemp -d)
 tar -xzf "$task_sdk_root/dist/twr-go-sdk-$task_version.tar.gz" -C "$task_project"
 (
  cd "$task_project"
  go mod init "$task_name"
  go mod edit -require=github.com/tarantool/tarantool-wasm-rs/guests/go@v0.1.0 \
   -replace=github.com/tarantool/tarantool-wasm-rs/guests/go=./twr-go-sdk-0.1.0
  cat > main.go <<'GO'
package main
import (
 "fmt"
 twr "github.com/tarantool/tarantool-wasm-rs/guests/go"
 "github.com/tarantool/tarantool-wasm-rs/guests/go/codec"
)
func main() {
 twr.Box.Space("users").Index("primary") // Construction must not call the host.
 b, err := twr.Pack(codec.Decimal{Digits:"12345678901234567890",Scale:5})
 if err != nil { panic(err) }
 if _, err := twr.Unpack(b); err != nil { panic(err) }
 fmt.Println("installed SDK ok")
}
GO
  go mod tidy
  go run .
 )
 rm -rf "$task_project"
done
