package luacallback

import (
	"errors"
	"testing"

	twr "github.com/tarantool/tarantool-wasm-rs/guests/go"
	"github.com/tarantool/tarantool-wasm-rs/guests/go/codec"
)

type fake struct{ closed int }

func (f *fake) Call(op string, a ...any) (any, error) {
	switch op {
	case "Open":
		return uint32(1), nil
	case "Close":
		f.closed++
		return nil, nil
	case "Call":
		return a[1], nil
	}
	return nil, &Error{"unknown operation"}
}
func TestCallbackOwnership(t *testing.T) {
	f := &fake{}
	c, e := OpenWithBindings(f, "echo")
	if e != nil {
		t.Fatal(e)
	}
	copy := *c
	v, e := c.Call(codec.Decimal{Digits: "12345678901234567890", Scale: 4})
	if e != nil || v.(codec.Decimal).Digits != "12345678901234567890" {
		t.Fatal(v, e)
	}
	c.Close()
	copy.Close()
	if f.closed != 1 {
		t.Fatal(f.closed)
	}
	_, e = copy.Call(nil)
	if !errors.Is(e, twr.ErrClosed) {
		t.Fatal(e)
	}
}
func TestUsingPanic(t *testing.T) {
	f := &fake{}
	c, e := OpenWithBindings(f, "echo")
	if e != nil {
		t.Fatal(e)
	}
	func() {
		defer func() {
			if recover() != "original" {
				t.Fatal("panic lost")
			}
		}()
		_ = twr.Using(c, func(*Callback) error { panic("original") })
	}()
	if f.closed != 1 {
		t.Fatal(f.closed)
	}
}
func TestCallbackError(t *testing.T) {
	e := &Error{"failure"}
	var out *Error
	if !errors.As(e, &out) || !errors.Is(e, &Error{}) {
		t.Fatal(e)
	}
}
