//go:build !wasm && !tinygo.wasm

package luacallback

import (
	"go.bytecodealliance.org/cm"
)

func wasmimport_HandlerResourceDrop(self0 uint32) { panic("Tarantool host imports require Wasm") }
func wasmimport_HandlerCall(self0 uint32, payload0 *uint8, payload1 uint32, result *cm.Result[cm.List[uint8], cm.List[uint8], string]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Open(name0 *uint8, name1 uint32, result *cm.Result[string, Handler, string]) {
	panic("Tarantool host imports require Wasm")
}
