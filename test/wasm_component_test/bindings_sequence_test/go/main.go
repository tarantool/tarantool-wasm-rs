package main

import (
    "go.bytecodealliance.org/cm"
    "adder-wasm-bindings/internal/docs/adder/adder"
    "adder-wasm-bindings/internal/tarantool/tarantool/sequence"
    "adder-wasm-bindings/internal/tarantool/tarantool/types"

)

type BoxError = types.BoxError
type BoxErrorShape = adder.BoxErrorShape
type Sequence = types.Sequence

func init() {
    adder.Exports.TNext = func(seq Sequence) (result cm.Result[BoxErrorShape, int64, BoxError]) {
        res := sequence.Next(seq)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, int64, BoxError], BoxErrorShape, int64, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, int64, BoxError], BoxErrorShape, int64, BoxError](*res.Err())
    }

	adder.Exports.TCurrent = func(seq Sequence) (result cm.Result[BoxErrorShape, int64, BoxError]) {
        res := sequence.Current(seq)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, int64, BoxError], BoxErrorShape, int64, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, int64, BoxError], BoxErrorShape, int64, BoxError](*res.Err())
    }

	adder.Exports.TSet = func(seq Sequence, value int64) (result cm.Result[BoxError, struct{}, BoxError]) {
        res := sequence.Set(seq, value)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxError, struct{}, BoxError], BoxError, struct{}, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxError, struct{}, BoxError], BoxError, struct{}, BoxError](*res.Err())
    }

	adder.Exports.TReset = func(seq Sequence) (result cm.Result[BoxError, struct{}, BoxError]) {
        res := sequence.Reset(seq)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxError, struct{}, BoxError], BoxError, struct{}, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxError, struct{}, BoxError], BoxError, struct{}, BoxError](*res.Err())
    }
}

func main() {}
