package codec

import (
	"encoding/hex"
	"math"
	"reflect"
	"strings"
	"testing"
)

func TestGoldenValues(t *testing.T) {
	tests := []struct {
		v   any
		hex string
	}{
		{nil, "c0"}, {[]byte(nil), "c0"}, {[]any(nil), "c0"}, {map[string]any(nil), "c0"}, {(*int)(nil), "c0"},
		{[]byte{}, "c400"}, {[]any{}, "90"}, {map[string]any{}, "80"},
		{uint64(math.MaxUint64), "cfffffffffffffffff"}, {int64(math.MinInt64), "d38000000000000000"}, {int64(math.MaxInt64), "cf7fffffffffffffff"},
		{Decimal{"12345", 2, true}, "d6010212345d"},
	}
	for _, tt := range tests {
		b, e := Pack(tt.v)
		if e != nil {
			t.Fatal(e)
		}
		if got := hex.EncodeToString(b); got != tt.hex {
			t.Errorf("%#v: %s != %s", tt.v, got, tt.hex)
		}
		if _, e := Unpack(b); e != nil {
			t.Fatal(e)
		}
	}
}
func TestExtensions(t *testing.T) {
	values := []any{UUID{1, 2, 3}, Decimal{"12345678901234567890123456789012345678", -20, true}, Datetime{-1, 999999999, -180, 42}, Interval{Years: -2, Nanoseconds: 999, Adjust: 2}, Extension{-7, []byte{1, 2, 3}}, ErrorValue{Stack: []ErrorFrame{{Type: "ClientError", File: "file", Message: "boom", Code: 999, Fields: map[string]any{"binary": []byte{0, 1}}, Extra: map[uint64]any{8: "future"}}}, Extra: map[uint64]any{99: "attribute"}}}
	for _, v := range values {
		b, e := Pack(v)
		if e != nil {
			t.Fatal(e)
		}
		out, e := Unpack(b)
		if e != nil {
			t.Fatal(e)
		}
		again, e := Pack(out)
		if e != nil {
			t.Fatal(e)
		}
		decoded, e := Unpack(again)
		if e != nil {
			t.Fatal(e)
		}
		if !reflect.DeepEqual(out, decoded) {
			t.Fatalf("extension roundtrip: %#v != %#v", out, decoded)
		}
	}
}
func TestMaps(t *testing.T) {
	values := []any{map[string]any{"a": uint64(1)}, map[any]any{uint64(2): "b", nil: "nil"}, Pairs{{[]any{uint64(1)}, "array"}, {[]byte{1}, "binary"}}, Pairs{{"same", uint64(1)}, {"same", uint64(2)}}}
	for _, v := range values {
		b, e := Pack(v)
		if e != nil {
			t.Fatal(e)
		}
		out, e := Unpack(b)
		if e != nil {
			t.Fatal(e)
		}
		b2, e := Pack(out)
		if e != nil {
			t.Fatal(e)
		}
		out2, e := Unpack(b2)
		if e != nil {
			t.Fatal(e)
		}
		if !reflect.DeepEqual(out, out2) {
			t.Fatal(out, out2)
		}
	}
}
func TestInvalid(t *testing.T) {
	a := []any{nil}
	a[0] = a
	m := map[string]any{}
	m["self"] = m
	for _, v := range []any{a, m, func() {}, make(chan int), struct{ X int }{1}, "\xff", Decimal{"NaN", 0, false}, Datetime{Nanoseconds: -1}, Interval{Adjust: 3}} {
		if _, e := Pack(v); e == nil {
			t.Fatalf("accepted %T", v)
		}
	}
	for _, b := range [][]byte{{0xc1}, {0xdb, 0xff, 0xff, 0xff, 0xff}, {0x90, 0x90}, {0xc7, 2, 2, 1, 2}, {0xc7, 3, 6, 1, 0, 0xc0}, {0xd4, 1, 0xff}, {0xa1, 0xff}} {
		if _, e := Unpack(b); e == nil {
			t.Fatalf("accepted %x", b)
		}
	}
	if _, e := Pack(a); e == nil || !strings.Contains(e.Error(), "cyclic") {
		t.Fatal(e)
	}
}
func TestIntegerWireTypes(t *testing.T) {
	for _, n := range []uint64{0, 127, 128, 255, 256, 65535, 65536, math.MaxUint32, 1 << 53, math.MaxUint64} {
		b, e := Pack(n)
		if e != nil {
			t.Fatal(e)
		}
		v, e := Unpack(b)
		if e != nil || v != n {
			t.Fatal(n, v, e)
		}
	}
	for _, n := range []int64{-1, -32, -33, -128, -129, -32768, -32769, math.MinInt32, math.MinInt64} {
		b, _ := Pack(n)
		v, e := Unpack(b)
		if e != nil || v != n {
			t.Fatal(n, v, e)
		}
	}
}
func FuzzUnpack(f *testing.F) {
	for _, b := range [][]byte{{0xc0}, {0x90}, {0x80}, {0xcf, 1}, {0xd8, 2}} {
		f.Add(b)
	}
	f.Fuzz(func(t *testing.T, b []byte) {
		v, e := Unpack(b)
		if e != nil {
			return
		}
		out, e := Pack(v)
		if e != nil {
			t.Fatal(e)
		}
		if _, e := Unpack(out); e != nil {
			t.Fatal(e)
		}
	})
}
