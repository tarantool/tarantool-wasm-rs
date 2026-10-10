//go:build !wasm && !tinygo.wasm

// Host imports are unavailable outside Wasm; use test bindings.
package index

import (
	"go.bytecodealliance.org/cm"
)

func wasmimport_IteratorResourceDrop(self0 uint32) { panic("Tarantool host imports require Wasm") }
func wasmimport_IteratorNew(index0 uint32, index1 uint32, mode0 uint32, key0 *uint8, key1 uint32, result *cm.Result[BoxErrorShape, Iterator, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_IteratorNext(self0 uint32, result *cm.Result[BoxErrorShape, cm.Option[BoxTuple], BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_TupleCount(index0 uint32, index1 uint32, result *cm.Result[BoxErrorShape, uint64, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_MemorySize(index0 uint32, index1 uint32, result *cm.Result[BoxErrorShape, uint64, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Random(index0 uint32, index1 uint32, rnd0 uint32, result *cm.Result[BoxErrorShape, cm.Option[BoxTuple], BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Get(index0 uint32, index1 uint32, key0 *uint8, key1 uint32, result *cm.Result[BoxErrorShape, cm.Option[BoxTuple], BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Min(index0 uint32, index1 uint32, key0 *uint8, key1 uint32, result *cm.Result[BoxErrorShape, cm.Option[BoxTuple], BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Max(index0 uint32, index1 uint32, key0 *uint8, key1 uint32, result *cm.Result[BoxErrorShape, cm.Option[BoxTuple], BoxError]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Count(index0 uint32, index1 uint32, mode0 uint32, key0 *uint8, key1 uint32, result *cm.Result[BoxErrorShape, uint64, BoxError]) {
	panic("Tarantool host imports require Wasm")
}
