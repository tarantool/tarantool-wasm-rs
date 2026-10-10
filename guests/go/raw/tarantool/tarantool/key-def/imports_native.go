//go:build !wasm && !tinygo.wasm

// Host imports are unavailable outside Wasm; use test bindings.
package keydef

import (
	"go.bytecodealliance.org/cm"
)

func wasmimport_New(parts0 *KeyPart, parts1 uint32, result *cm.Result[BoxErrorShape, KeyDef, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Duplicate(keyDef0 uint64, result *cm.Result[BoxErrorShape, KeyDef, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Merge(left0 uint64, right0 uint64, result *cm.Result[BoxErrorShape, KeyDef, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Release(keyDef0 uint64) { panic("Tarantool host imports require Wasm") }
func wasmimport_DumpParts(keyDef0 uint64, result *cm.Result[BoxErrorShape, cm.List[KeyPart], BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_ValidateKey(keyDef0 uint64, key0 *uint8, key1 uint32, result *cm.Result[BoxError, struct{}, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_ValidateFullKey(keyDef0 uint64, key0 *uint8, key1 uint32, result *cm.Result[BoxError, struct{}, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_PartCount(keyDef0 uint64, result *cm.Result[BoxErrorShape, uint32, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_ExtractKey(keyDef0 uint64, tuple0 uint64, result *cm.Result[BoxErrorShape, MsgpackValue, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_ValidateTuple(keyDef0 uint64, tuple0 uint64, result *cm.Result[BoxError, struct{}, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
