//go:build !wasm && !tinygo.wasm

// Host imports are unavailable outside Wasm; use test bindings.
package msgpack

import (
	"go.bytecodealliance.org/cm"
)

func wasmimport_FromJSON(json0 *uint8, json1 uint32, result *cm.Result[BoxErrorShape, MsgpackValue, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_ToJSON(value0 *uint8, value1 uint32, result *cm.Result[BoxErrorShape, string, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
