#!/bin/sh
set -e
# wkg wit build
go run go.bytecodealliance.org/cmd/wit-bindgen-go generate -o internal/ ./docs:adder@0.1.0.wasm
tinygo build -target=wasip2 -o add.wasm --wit-package docs:adder@0.1.0.wasm --wit-world adder main.go
wasm-tools component wit docs:adder@0.1.0.wasm > adder-bundle.wit
