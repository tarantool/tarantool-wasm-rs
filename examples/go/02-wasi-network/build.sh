#!/bin/sh
set -e
wit-bindgen-go generate -o internal/ -w adder ./docs:adder@0.1.0.wasm
tinygo build -target=wasip2 -o go.wasm --wit-package docs:adder@0.1.0.wasm --wit-world adder main.go
wasm-tools component wit docs:adder@0.1.0.wasm > adder-bundle.wit
