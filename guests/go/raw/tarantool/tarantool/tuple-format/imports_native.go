//go:build !wasm && !tinygo.wasm

// Host imports are unavailable outside Wasm; use test bindings.
package tupleformat

import (
	"go.bytecodealliance.org/cm"
)

func wasmimport_Default() (result0 uint64) { panic("Tarantool host imports require Wasm") }
func wasmimport_New(keyDefs0 *KeyDef, keyDefs1 uint32, result *cm.Result[BoxErrorShape, TupleFormat, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Retain(tf0 uint64)  { panic("Tarantool host imports require Wasm") }
func wasmimport_Release(tf0 uint64) { panic("Tarantool host imports require Wasm") }
