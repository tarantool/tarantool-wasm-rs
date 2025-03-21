#!/bin/sh
rm -rf adder
componentize-py --wit-path ../../adder.wit --world adder bindings .
componentize-py --wit-path ../../adder.wit --world adder componentize app -o py.wasm
