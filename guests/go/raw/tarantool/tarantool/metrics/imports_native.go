//go:build !wasm && !tinygo.wasm

// Host imports are unavailable outside Wasm; use test bindings.
package metrics

func wasmimport_Uptime() (result0 float64) { panic("Tarantool host imports require Wasm") }
