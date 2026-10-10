//go:build !wasm && !tinygo.wasm

// Host imports are unavailable outside Wasm; use test bindings.
package session

import (
	"go.bytecodealliance.org/cm"
)

func wasmimport_Current() (result0 uint64) { panic("Tarantool host imports require Wasm") }
func wasmimport_Broadcast(key0 *uint8, key1 uint32, value0 *uint8, value1 uint32, result *cm.Result[BoxError, struct{}, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_IprotoSend(session0 uint64, header0 *uint8, header1 uint32, body0 uint32, body1 *uint8, body2 uint32, result *cm.Result[BoxError, struct{}, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
