package main

import (
    "adder-wasm-bindings/internal/docs/adder/adder"
    "go.bytecodealliance.org/cm"
    error "adder-wasm-bindings/internal/tarantool/tarantool/error"
    "adder-wasm-bindings/internal/tarantool/tarantool/types"
)

func main() {}

type BoxError = types.BoxError
type ErrorCode = types.ErrorCode

func init() {

    adder.Exports.TNew = func(code ErrorCode, message string) (result BoxError) {
        return error.New(code, message)
    }

    adder.Exports.TNewWithLocation = func(code ErrorCode, message string, file cm.Option[string], line cm.Option[uint32]) (result BoxError) {
        return error.NewWithLocation(code, message, file, line)
    }

	adder.Exports.TSet = func(err BoxError) {
        error.Set(err)
    }

	adder.Exports.TLast = func() (result cm.Option[BoxError]) {
        return error.Last()
    }

	adder.Exports.TClear = func() {
        error.Clear()
    }

	adder.Exports.TToString = func(err BoxError) (result string) {
        return error.ToString(err)
    }

}
