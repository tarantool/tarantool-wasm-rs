#!/bin/sh
jco componentize main.js \
    --wit ../../adder.wit \
    --world-name adder \
    --out js.wasm --disable http
