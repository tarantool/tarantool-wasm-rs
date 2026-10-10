//go:build !wasm && !tinygo.wasm

// Host imports are unavailable outside Wasm; use test bindings.
package database

import (
	"go.bytecodealliance.org/cm"
)

func wasmimport_SchemaVersion() (result0 uint32) { panic("Tarantool host imports require Wasm") }
func wasmimport_SpaceByName(name0 *uint8, name1 uint32, result *cm.Result[BoxErrorShape, cm.Option[Space], BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_IndexByName(space0 uint32, name0 *uint8, name1 uint32, result *cm.Result[BoxErrorShape, cm.Option[Index], BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Insert(space0 uint32, tup0 *uint8, tup1 uint32, result *cm.Result[BoxErrorShape, BoxTuple, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Update(index0 uint32, index1 uint32, key0 *uint8, key1 uint32, ops0 *uint8, ops1 uint32, result *cm.Result[BoxErrorShape, cm.Option[BoxTuple], BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Replace(space0 uint32, tup0 *uint8, tup1 uint32, result *cm.Result[BoxErrorShape, BoxTuple, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Upsert(index0 uint32, index1 uint32, tup0 *uint8, tup1 uint32, ops0 *uint8, ops1 uint32, result *cm.Result[BoxError, struct{}, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Delete(index0 uint32, index1 uint32, key0 *uint8, key1 uint32, result *cm.Result[BoxErrorShape, cm.Option[BoxTuple], BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Truncate(space0 uint32, result *cm.Result[BoxError, struct{}, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
