//go:build !wasm && !tinygo.wasm

package zlib

import (
	"go.bytecodealliance.org/cm"
)

func wasmimport_Compress(data0 *uint8, data1 uint32, level0 uint32, result *cm.List[uint8]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Decompress(data0 *uint8, data1 uint32, result *cm.List[uint8]) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Crc32(data0 *uint8, data1 uint32) (result0 uint32) {
	panic("Tarantool host imports require Wasm")
}
func wasmimport_Adler32(data0 *uint8, data1 uint32) (result0 uint32) {
	panic("Tarantool host imports require Wasm")
}
