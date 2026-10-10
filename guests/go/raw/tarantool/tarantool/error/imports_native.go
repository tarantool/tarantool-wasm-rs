//go:build !wasm && !tinygo.wasm

// Host imports are unavailable outside Wasm; use test bindings.
package tarantoolerror

import (
	"go.bytecodealliance.org/cm"
)

func wasmimport_New(code0 uint32, message0 *uint8, message1 uint32, result *BoxError) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_NewWithLocation(code0 uint32, message0 *uint8, message1 uint32, file0 uint32, file1 *uint8, file2 uint32, line0 uint32, line1 uint32, result *BoxError) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Set(err0 uint32, err1 *uint8, err2 uint32, err3 *uint8, err4 uint32, err5 uint32, err6 *uint8, err7 uint32, err8 uint32, err9 uint32) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Last(result *cm.Option[BoxError]) { panic("Tarantool host imports require Wasm") }
func wasmimport_Clear()                           { panic("Tarantool host imports require Wasm") }
func wasmimport_ToString(err0 uint32, err1 *uint8, err2 uint32, err3 *uint8, err4 uint32, err5 uint32, err6 *uint8, err7 uint32, err8 uint32, err9 uint32, result *string) {
	panic("Tarantool host imports require Wasm")
}
