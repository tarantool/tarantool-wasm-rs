package main

import (
	world "another/go-extras/internal/another/go-extras/v0.1.0/sdk-test"
	"another/go-extras/internal/wasi/cli/v0.2.12/run"
	"bytes"
	"errors"
	"fmt"
	"hash/adler32"
	"hash/crc32"
	"math"
	"reflect"

	twr "github.com/tarantool/tarantool-wasm-rs/guests/go"
	"github.com/tarantool/tarantool-wasm-rs/guests/go/codec"
	"github.com/tarantool/tarantool-wasm-rs/guests/go/luacallback"
	zlib "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/docs/adder/zlib"
	"go.bytecodealliance.org/cm"
)

func must(e error) {
	if e != nil {
		panic(e)
	}
}
func assert(ok bool) {
	if !ok {
		panic("assertion failed")
	}
}
func check(name string) (result string) {
	defer func() {
		if p := recover(); p != nil {
			result = fmt.Sprint(p)
		}
	}()
	switch name {
	case "lua-callback":
		for _, v := range []any{nil, []byte{0, 255}, uint64(math.MaxUint64), int64(math.MinInt64), codec.Decimal{Digits: "12345678901234567890123456789012345678", Scale: 10}, codec.UUID{1, 2}, codec.Datetime{Seconds: 123, Nanoseconds: 999999999, TZIndex: 42, TZOffset: 180}, codec.Interval{Days: -1, Adjust: 2}, codec.ErrorValue{Stack: []codec.ErrorFrame{{Type: "ClientError", Message: "go", Code: 42}}}} {
			must(luacallback.Using("go_echo", func(c *luacallback.Callback) error {
				out, e := c.Call(v)
				if e != nil {
					return e
				}
				b, e := twr.Pack(v)
				if e != nil {
					return e
				}
				b2, e := twr.Pack(out)
				if e != nil {
					return e
				}
				expected, e := twr.Unpack(b)
				if e != nil {
					return e
				}
				got, e := twr.Unpack(b2)
				if e != nil {
					return e
				}
				if !reflect.DeepEqual(expected, got) {
					return fmt.Errorf("callback value mismatch: %#v vs %#v", expected, got)
				}
				return nil
			}))
		}
		c, e := luacallback.Open("go_fail")
		must(e)
		_, e = c.Call(nil)
		var ce *luacallback.Error
		assert(errors.As(e, &ce))
		assert(errors.Is(e, &luacallback.Error{}))
		must(c.Close())
		must(c.Close())
		_, e = c.Call(nil)
		assert(errors.Is(e, twr.ErrClosed))
		must(luacallback.Using("go_binary", func(c *luacallback.Callback) error { _, e := c.Call([]byte{1}); return e }))
		func() {
			defer func() { assert(recover() == "callback panic") }()
			_ = luacallback.Using("go_echo", func(c *luacallback.Callback) error { panic("callback panic") })
		}()
		var nilPanicCallback *luacallback.Callback
		func() {
			returned := false
			defer func() { _ = recover(); assert(!returned) }()
			_ = luacallback.Using("go_echo", func(c *luacallback.Callback) error {
				nilPanicCallback = c
				panic(nil)
			})
			returned = true
		}()
		_, e = nilPanicCallback.Call(nil)
		assert(errors.Is(e, twr.ErrClosed))
		must(luacallback.Using("go_replace", func(c *luacallback.Callback) error {
			v, e := c.Call(uint64(10))
			if e != nil {
				return e
			}
			assert(v == uint64(11))
			v, e = c.Call(uint64(10))
			if e != nil {
				return e
			}
			assert(v == uint64(12))
			return nil
		}))
	case "zlib":
		data := []byte("hello from Go")
		b := cm.ToList(data)
		compressed := zlib.Compress(b, 6)
		assert(bytes.Equal(zlib.Decompress(compressed).Slice(), data))
		assert(zlib.Crc32(b) == crc32.ChecksumIEEE(data))
		assert(zlib.Adler32(b) == adler32.Checksum(data))
	default:
		return "unknown check " + name
	}
	return "ok"
}
func init() {
	world.Exports.Check = check
	world.Exports.Roundtrip = func(b cm.List[uint8]) cm.List[uint8] { return b }
	run.Exports.Run = func() cm.BoolResult { return 0 }
}
func main() {}
