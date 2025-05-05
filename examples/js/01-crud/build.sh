#!/bin/sh
jco guest-types -o internal ../../../wit
jco componentize main.js \
    --wit ../../../wit \
    --world-name adder \
    --out js.wasm --disable http
