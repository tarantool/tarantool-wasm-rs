package main

import (
    "go.bytecodealliance.org/cm"
    "adder-wasm-bindings/internal/docs/adder/adder"
    kd "adder-wasm-bindings/internal/tarantool/tarantool/key-def"
    boxTuple "adder-wasm-bindings/internal/tarantool/tarantool/box-tuple"
    "adder-wasm-bindings/internal/tarantool/tarantool/msgpack"
    "adder-wasm-bindings/internal/tarantool/tarantool/types"
)

type BoxErrorShape = adder.BoxErrorShape
type BoxTuple = types.BoxTuple
type BoxError = types.BoxError
type KeyDef = types.KeyDef
type KeyPart = types.KeyPart

func init() {
	adder.Exports.TTupleNew = func(data cm.List[uint8]) (result cm.Result[BoxErrorShape, BoxTuple, BoxError]) {
        res := boxTuple.New(data)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, BoxTuple, BoxError], BoxErrorShape, BoxTuple, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, BoxTuple, BoxError], BoxErrorShape, BoxTuple, BoxError](*res.Err())
    }

	adder.Exports.TNew = func(parts cm.List[KeyPart]) (result cm.Result[BoxErrorShape, KeyDef, BoxError]) {
        res := kd.New(parts)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, KeyDef, BoxError], BoxErrorShape, KeyDef, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, KeyDef, BoxError], BoxErrorShape, KeyDef, BoxError](*res.Err())
    }

	adder.Exports.TDup = func(keyDef KeyDef) (result cm.Result[BoxErrorShape, KeyDef, BoxError]) {
        res := kd.Dup(keyDef)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, KeyDef, BoxError], BoxErrorShape, KeyDef, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, KeyDef, BoxError], BoxErrorShape, KeyDef, BoxError](*res.Err())
    }

	adder.Exports.TMerge = func(left KeyDef, right KeyDef) (result cm.Result[BoxErrorShape, KeyDef, BoxError]) {
        res := kd.Merge(left, right)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, KeyDef, BoxError], BoxErrorShape, KeyDef, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, KeyDef, BoxError], BoxErrorShape, KeyDef, BoxError](*res.Err())
    }

	adder.Exports.TDelete = func(keyDef KeyDef) {
        kd.Delete(keyDef)
    }

	adder.Exports.TDumpParts = func(keyDef KeyDef) (result cm.Result[BoxErrorShape, cm.List[KeyPart], BoxError]) {
        res := kd.DumpParts(keyDef)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, cm.List[KeyPart], BoxError], BoxErrorShape, cm.List[KeyPart], BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, cm.List[KeyPart], BoxError], BoxErrorShape, cm.List[KeyPart], BoxError](*res.Err())
    }

	adder.Exports.TValidateKey = func(keyDef KeyDef, key cm.List[uint8]) (result cm.Result[BoxErrorShape, cm.Tuple[bool, uint32], BoxError]) {
        res := kd.ValidateKey(keyDef, key)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, cm.Tuple[bool, uint32], BoxError], BoxErrorShape, cm.Tuple[bool, uint32], BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, cm.Tuple[bool, uint32], BoxError], BoxErrorShape, cm.Tuple[bool, uint32], BoxError](*res.Err())
    }

	adder.Exports.TValidateFullKey = func(keyDef KeyDef, key cm.List[uint8]) (result cm.Result[BoxErrorShape, cm.Tuple[bool, uint32], BoxError]) {
        res := kd.ValidateFullKey(keyDef, key)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, cm.Tuple[bool, uint32], BoxError], BoxErrorShape, cm.Tuple[bool, uint32], BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, cm.Tuple[bool, uint32], BoxError], BoxErrorShape, cm.Tuple[bool, uint32], BoxError](*res.Err())
    }

	adder.Exports.TPartCount = func(keyDef KeyDef) (result cm.Result[BoxErrorShape, uint32, BoxError]) {
        res := kd.PartCount(keyDef)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, uint32, BoxError], BoxErrorShape, uint32, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, uint32, BoxError], BoxErrorShape, uint32, BoxError](*res.Err())
    }

	adder.Exports.TExtractKey = func(keyDef KeyDef, tuple BoxTuple) (result cm.Result[BoxErrorShape, cm.List[uint8], BoxError]) {
        res := kd.ExtractKey(keyDef, tuple)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res.Err())
    }

	adder.Exports.TValidateTuple = func(keyDef KeyDef, tuple BoxTuple) (result bool) {
        return kd.ValidateTuple(keyDef, tuple)
    }

	adder.Exports.DecodeFromRawPtr = func(ptr uint64) (result cm.List[uint8]) {
        res := msgpack.DecodeFromRawPtr(ptr)
        if res.IsOK() {
            return *res.OK()
        }
        panic("DecodeFromRawPtr failed")
    }
}


func main() {}
