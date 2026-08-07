package main

import (
    "go.bytecodealliance.org/cm"
    "adder-wasm-bindings/internal/docs/adder/adder"
    kd "adder-wasm-bindings/internal/tarantool/tarantool/key-def"
    tupleFormat "adder-wasm-bindings/internal/tarantool/tarantool/tuple-format"
    "adder-wasm-bindings/internal/tarantool/tarantool/types"
)

type BoxErrorShape = adder.BoxErrorShape
type BoxError = types.BoxError
type KeyDef = types.KeyDef
type KeyPart = types.KeyPart
type TupleFormat = types.TupleFormat


func init() {

	adder.Exports.TKeyDefNew = func(parts cm.List[KeyPart]) (result cm.Result[BoxErrorShape, KeyDef, BoxError]) {
        res := kd.New(parts)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, KeyDef, BoxError], BoxErrorShape, KeyDef, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, KeyDef, BoxError], BoxErrorShape, KeyDef, BoxError](*res.Err())
    }

    adder.Exports.TDefault = func() (result TupleFormat) {
        return tupleFormat.Default()
    }

    adder.Exports.TNew = func(keyDefs cm.List[KeyDef]) (result cm.Result[BoxErrorShape, TupleFormat, BoxError]) {
        res := tupleFormat.New(keyDefs)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, TupleFormat, BoxError], BoxErrorShape, TupleFormat, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, TupleFormat, BoxError], BoxErrorShape, TupleFormat, BoxError](*res.Err())
    }

    adder.Exports.TRef = func(tf TupleFormat) {
        tupleFormat.Retain(tf)
    }

    adder.Exports.TUnref = func(tf TupleFormat) {
        tupleFormat.Release(tf)
    }
}


func main() {}
