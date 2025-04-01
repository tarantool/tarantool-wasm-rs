#!/bin/sh

rm -rf adder
componentize-py --wit-path ../../../adder.wit --world adder bindings .

for file in *.py; do
    [ -e "$file" ] || continue

    name=$(basename "$file" .py)
    output="${name}.wasm"

    echo "$name -> $output"
    componentize-py --wit-path ../../../adder.wit --world adder componentize $name -o $output
done
