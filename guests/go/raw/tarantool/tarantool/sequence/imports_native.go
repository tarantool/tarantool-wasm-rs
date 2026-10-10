//go:build !wasm && !tinygo.wasm

// Host imports are unavailable outside Wasm; use test bindings.
package sequence

import (
	"go.bytecodealliance.org/cm"
)

func wasmimport_Next(seq0 uint32, result *cm.Result[BoxErrorShape, int64, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Current(seq0 uint32, result *cm.Result[BoxErrorShape, int64, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Set(seq0 uint32, value0 uint64, result *cm.Result[BoxError, struct{}, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Reset(seq0 uint32, result *cm.Result[BoxError, struct{}, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
