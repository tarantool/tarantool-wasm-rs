package main

import (
    "adder-wasm-bindings/internal/docs/adder/adder"
    "go.bytecodealliance.org/cm"
)

func main() {}

func init() {

	adder.Exports.TBool = func(x bool) bool {
		return x
	}

	adder.Exports.TS8 = func(x int8) int8 {
		return x
	}

	adder.Exports.TS16 = func(x int16) int16 {
		return x
	}

	adder.Exports.TS32 = func(x int32) int32 {
		return x
	}

	adder.Exports.TS64 = func(x int64) int64 {
		return x
	}

	adder.Exports.TU8 = func(x uint8) uint8 {
		return x
	}

	adder.Exports.TU16 = func(x uint16) uint16 {
		return x
	}

	adder.Exports.TU32 = func(x uint32) uint32 {
		return x
	}

	adder.Exports.TU64 = func(x uint64) uint64 {
		return x
	}

	adder.Exports.TF32 = func(x float32) float32 {
		return x
	}

	adder.Exports.TF64 = func(x float64) float64 {
		return x
	}

	adder.Exports.TChar = func(x rune) rune {
		return x
	}

	adder.Exports.TString = func(x string) string {
		return x
	}

	adder.Exports.TList = func(x cm.List[string]) cm.List[string] {
		return x
	}

	adder.Exports.TOption = func(x cm.Option[uint8]) cm.Option[uint8] {
		return x
	}

	adder.Exports.TResult = func(x bool) cm.Result[string, bool, string] {
        if x {
            return cm.OK[cm.Result[string, bool, string], string, bool, string](x)
        } else {
            return cm.Err[cm.Result[string, bool, string], string, bool, string]("error")
        }
    }

	adder.Exports.TTuple = func(x cm.Tuple3[bool, float32, string]) cm.Tuple[bool, string] {
		return cm.Tuple[bool, string]{F0: x.F0, F1: x.F2}
	}

	adder.Exports.TRecord = func(x adder.R) adder.R {
		return x
	}

	adder.Exports.TVariant = func(x adder.V) adder.V {
		return x
	}

	adder.Exports.TEnum = func(x adder.E) adder.E {
		return x
	}

	adder.Exports.TFlags = func(x adder.F) adder.F {
		return x
	}
}
