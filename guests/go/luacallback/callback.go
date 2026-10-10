// Package luacallback is the optional MessagePack Lua callback API.
// Import it only in components whose world imports docs:adder/lua-callback.
package luacallback

import (
	"fmt"

	twr "github.com/tarantool/tarantool-wasm-rs/guests/go"
	"github.com/tarantool/tarantool-wasm-rs/guests/go/bindings"
	"github.com/tarantool/tarantool-wasm-rs/guests/go/codec"
	raw "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/docs/adder/lua-callback"
	"go.bytecodealliance.org/cm"
)

// Error is a callback failure, separate from Tarantool box errors.
type Error struct{ Message string }

func (e *Error) Error() string { return "Lua callback: " + e.Message }
func (e *Error) Is(target error) bool {
	t, ok := target.(*Error)
	return ok && (t.Message == "" || t.Message == e.Message)
}

// Host adapts only the optional callback imports.
type Host struct{}

func (Host) Call(op string, a ...any) (any, error) {
	switch op {
	case "Open":
		r := raw.Open(a[0].(string))
		if e := r.Err(); e != nil {
			return nil, &Error{*e}
		}
		return *r.OK(), nil
	case "Call":
		r := a[0].(raw.Handler).Call(cm.ToList(a[1].([]byte)))
		if e := r.Err(); e != nil {
			return nil, &Error{*e}
		}
		return r.OK().Slice(), nil
	case "Close":
		a[0].(raw.Handler).ResourceDrop()
		return nil, nil
	}
	return nil, fmt.Errorf("unknown callback operation %s", op)
}

type state struct {
	host   bindings.Caller
	handle any
	closed bool
}

// Callback owns one handler; copies share its closed state.
type Callback struct{ state *state }

func Open(name string) (*Callback, error) { return OpenWithBindings(Host{}, name) }
func OpenWithBindings(host bindings.Caller, name string) (*Callback, error) {
	h, e := host.Call("Open", name)
	if e != nil {
		return nil, e
	}
	return &Callback{&state{host: host, handle: h}}, nil
}
func (c *Callback) Call(v any) (any, error) {
	if c.state == nil || c.state.closed {
		return nil, twr.ErrClosed
	}
	b, e := codec.Pack(v)
	if e != nil {
		return nil, e
	}
	result, e := c.state.host.Call("Call", c.state.handle, b)
	if e != nil {
		return nil, e
	}
	data, ok := result.([]byte)
	if !ok {
		return nil, fmt.Errorf("callback binding returned %T", result)
	}
	return codec.Unpack(data)
}
func (c *Callback) Close() error {
	if c.state == nil || c.state.closed {
		return nil
	}
	c.state.closed = true
	_, e := c.state.host.Call("Close", c.state.handle)
	return e
}

// Using opens a named callback and closes it on return, error and panic.
// Lua registers/replaces callbacks explicitly through the runtime API.
func Using(name string, fn func(*Callback) error) error {
	c, e := Open(name)
	if e != nil {
		return e
	}
	return twr.Using(c, fn)
}
