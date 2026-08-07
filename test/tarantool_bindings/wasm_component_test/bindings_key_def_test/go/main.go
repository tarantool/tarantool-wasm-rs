package main

import (
	"adder-wasm-bindings/internal/docs/adder/adder"
	boxTuple "adder-wasm-bindings/internal/tarantool/tarantool/box-tuple"
	kd "adder-wasm-bindings/internal/tarantool/tarantool/key-def"
	"adder-wasm-bindings/internal/tarantool/tarantool/types"
	"go.bytecodealliance.org/cm"
)

type BoxErrorShape = adder.BoxErrorShape
type BoxTuple = types.BoxTuple
type BoxError = types.BoxError
type KeyDef = types.KeyDef
type KeyPart = types.KeyPart
type MsgpackValue = types.MsgpackValue

func convertResult[FromShape, ToShape, Value, Err any](
	result cm.Result[FromShape, Value, Err],
) cm.Result[ToShape, Value, Err] {
	if result.IsErr() {
		return cm.Err[cm.Result[ToShape, Value, Err], ToShape, Value, Err](*result.Err())
	}
	return cm.OK[cm.Result[ToShape, Value, Err], ToShape, Value, Err](*result.OK())
}

func init() {
	adder.Exports.TTupleNew = func(data MsgpackValue) cm.Result[BoxErrorShape, BoxTuple, BoxError] {
		return convertResult[boxTuple.BoxErrorShape, BoxErrorShape, BoxTuple, BoxError](boxTuple.New(data))
	}
	adder.Exports.TNew = func(parts cm.List[KeyPart]) cm.Result[BoxErrorShape, KeyDef, BoxError] {
		return convertResult[kd.BoxErrorShape, BoxErrorShape, KeyDef, BoxError](kd.New(parts))
	}
	adder.Exports.TDuplicate = func(keyDef KeyDef) cm.Result[BoxErrorShape, KeyDef, BoxError] {
		return convertResult[kd.BoxErrorShape, BoxErrorShape, KeyDef, BoxError](kd.Duplicate(keyDef))
	}
	adder.Exports.TMerge = func(left, right KeyDef) cm.Result[BoxErrorShape, KeyDef, BoxError] {
		return convertResult[kd.BoxErrorShape, BoxErrorShape, KeyDef, BoxError](kd.Merge(left, right))
	}
	adder.Exports.TRelease = kd.Release
	adder.Exports.TDumpParts = func(keyDef KeyDef) cm.Result[BoxErrorShape, cm.List[KeyPart], BoxError] {
		return convertResult[kd.BoxErrorShape, BoxErrorShape, cm.List[KeyPart], BoxError](kd.DumpParts(keyDef))
	}
	adder.Exports.TValidateKey = kd.ValidateKey
	adder.Exports.TValidateFullKey = kd.ValidateFullKey
	adder.Exports.TPartCount = func(keyDef KeyDef) cm.Result[BoxErrorShape, uint32, BoxError] {
		return convertResult[kd.BoxErrorShape, BoxErrorShape, uint32, BoxError](kd.PartCount(keyDef))
	}
	adder.Exports.TExtractKey = func(keyDef KeyDef, tuple BoxTuple) cm.Result[BoxErrorShape, MsgpackValue, BoxError] {
		return convertResult[kd.BoxErrorShape, BoxErrorShape, MsgpackValue, BoxError](kd.ExtractKey(keyDef, tuple))
	}
	adder.Exports.TValidateTuple = kd.ValidateTuple
}

func main() {}
