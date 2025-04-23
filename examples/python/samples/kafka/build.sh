#!/bin/sh

rm -rf adder
componentize-py --wit-path ../../../../wit --world adder bindings .
componentize-py --wit-path ../../../../wit --world adder componentize main -o kafka.wasm
