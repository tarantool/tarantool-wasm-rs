//go:build !wasm && !tinygo.wasm

// Host imports are unavailable outside Wasm; use test bindings.
package transaction

import (
	"go.bytecodealliance.org/cm"
)

func wasmimport_ID(result *cm.Option[uint64]) { panic("Tarantool host imports require Wasm") }
func wasmimport_Isolation(result *cm.Option[TransactionIsolation]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Begin(result *cm.Result[BoxError, struct{}, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Commit(result *cm.Result[BoxError, struct{}, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Rollback(result *cm.Result[BoxError, struct{}, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_SetIsolation(isolation0 uint32, result *cm.Result[BoxError, struct{}, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_MakeSynchronous() { panic("Tarantool host imports require Wasm") }
