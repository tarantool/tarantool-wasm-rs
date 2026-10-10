//go:build !wasm && !tinygo.wasm

// Host imports are unavailable outside Wasm; use test bindings.
package boxtuple

import (
	"go.bytecodealliance.org/cm"
)

func wasmimport_IteratorResourceDrop(self0 uint32) { panic("Tarantool host imports require Wasm") }
func wasmimport_IteratorNew(tuple0 uint64, result *cm.Result[BoxErrorShape, Iterator, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_IteratorNext(self0 uint32, result *cm.Result[BoxErrorShape, cm.Option[TupleField], BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_IteratorPosition(self0 uint32) (result0 uint32) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_IteratorRewind(self0 uint32) { panic("Tarantool host imports require Wasm") }
func wasmimport_IteratorSeek(self0 uint32, pos0 uint32, result *cm.Result[BoxErrorShape, cm.Option[TupleField], BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_New(data0 *uint8, data1 uint32, result *cm.Result[BoxErrorShape, BoxTuple, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Retain(t0 uint64)                      { panic("Tarantool host imports require Wasm") }
func wasmimport_Release(t0 uint64)                     { panic("Tarantool host imports require Wasm") }
func wasmimport_FieldCount(t0 uint64) (result0 uint32) { panic("Tarantool host imports require Wasm") }
func wasmimport_ByteSize(t0 uint64) (result0 uint64)   { panic("Tarantool host imports require Wasm") }
func wasmimport_ToBytes(t0 uint64, result *cm.Result[BoxErrorShape, MsgpackValue, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Format(t0 uint64) (result0 uint64) { panic("Tarantool host imports require Wasm") }
func wasmimport_Field(t0 uint64, idx0 uint32, result *cm.Result[BoxErrorShape, cm.Option[TupleField], BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_FieldByPath(t0 uint64, path0 *uint8, path1 uint32, result *cm.Result[BoxErrorShape, cm.Option[TupleField], BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Update(t0 uint64, expr0 *uint8, expr1 uint32, result *cm.Result[BoxErrorShape, BoxTuple, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Upsert(t0 uint64, expr0 *uint8, expr1 uint32, result *cm.Result[BoxErrorShape, BoxTuple, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Validate(t0 uint64, format0 uint64, result *cm.Result[BoxError, struct{}, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Compare(keyDef0 uint64, left0 uint64, right0 uint64) (result0 uint32) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_CompareWithKey(tuple0 uint64, key0 *uint8, key1 uint32, keyDef0 uint64, result *cm.Result[BoxErrorShape, int32, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
