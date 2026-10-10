//go:build !wasm && !tinygo.wasm

// Host imports are unavailable outside Wasm; use test bindings.
package log

func wasmimport_Write(level0 uint32, message0 *uint8, message1 uint32, context0 uint32, context1 *uint8, context2 uint32, context3 uint32) {
	panic("Tarantool host imports require Wasm")
}
