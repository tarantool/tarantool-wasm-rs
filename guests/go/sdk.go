// Package twr provides the Tarantool-Wasm-Rs Go SDK. Host calls happen only
// when a method is invoked. Resources require explicit Close, never finalizers.
package twr

import (
	"errors"
	"fmt"
	"io"
	"math"
	"reflect"

	"github.com/tarantool/tarantool-wasm-rs/guests/go/bindings"
	"github.com/tarantool/tarantool-wasm-rs/guests/go/codec"
	"github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/types"
)

var (
	ErrClosed             = errors.New("twr: resource is closed")
	ErrNotFound           = errors.New("twr: schema object not found")
	ErrNestedTransaction  = errors.New("twr: nested transaction")
	ErrManagedTransaction = errors.New("twr: manual completion inside transaction helper")
	ErrTransactionChanged = errors.New("twr: transaction ended or replaced inside helper")
)

type Error = bindings.Error

// Client groups API objects with one binding provider. It is confined to the
// current synchronous host execution; transaction goroutines are unsupported.
type Client struct {
	host    bindings.Caller
	managed bool
}

// Box is the default lazy client. Initialization creates no host resources.
var Box = New(bindings.Host{})

func New(host bindings.Caller) *Client { return &Client{host: host} }
func call[T any](c *Client, op string, args ...any) (T, error) {
	var zero T
	v, e := c.host.Call(op, args...)
	if e != nil {
		return zero, e
	}
	out, ok := v.(T)
	if !ok {
		return zero, fmt.Errorf("twr: binding %s returned %T", op, v)
	}
	return out, nil
}
func (c *Client) do(op string, args ...any) error { _, e := c.host.Call(op, args...); return e }
func (c *Client) SchemaVersion() (uint32, error)  { return call[uint32](c, "database.SchemaVersion") }
func Pack(v any) ([]byte, error)                  { return codec.Pack(v) }
func Unpack(b []byte) (any, error)                { return codec.Unpack(b) }
func array(v any) ([]byte, error) {
	if v == nil {
		return nil, fmt.Errorf("twr: expected non-nil tuple array")
	}
	k := reflect.TypeOf(v).Kind()
	if k != reflect.Array && k != reflect.Slice {
		return nil, fmt.Errorf("twr: expected tuple array")
	}
	if _, ok := v.([]byte); ok {
		return nil, fmt.Errorf("twr: binary is not a tuple array")
	}
	b, e := Pack(v)
	if e == nil && b[0] == 0xc0 {
		e = fmt.Errorf("twr: expected non-nil tuple array")
	}
	return b, e
}
func key(v any) ([]byte, error) {
	if v == nil {
		return Pack([]any{})
	}
	k := reflect.TypeOf(v).Kind()
	if (k == reflect.Array || k == reflect.Slice) && reflect.TypeOf(v).Elem().Kind() != reflect.Uint8 {
		return array(v)
	}
	return Pack([]any{v})
}

// Operation uses zero-based field numbers throughout the public SDK.
// Negative positions keep host semantics. Raw WIT retains native numbering.
type Operation struct {
	Operator  string
	Field     int64
	Arguments []any
}

func operations(ops []Operation, tupleAPI bool) ([]byte, error) {
	a := make([]any, len(ops))
	for i, o := range ops {
		if o.Operator == "" {
			return nil, fmt.Errorf("twr: empty update operator")
		}
		field := o.Field
		if tupleAPI && field >= 0 {
			if field == math.MaxInt64 {
				return nil, fmt.Errorf("twr: tuple operation field overflows")
			}
			field++
		}
		a[i] = append([]any{o.Operator, field}, o.Arguments...)
	}
	return Pack(a)
}

// Using guarantees Close on normal return, errors, early return and panic.
// A cleanup error is joined with the primary error. A primary panic is retained.
func Using[T io.Closer](r T, fn func(T) error) (err error) {
	returned := false
	defer func() {
		p := recover()
		var ce error
		func() {
			closed := false
			defer func() {
				if q := recover(); q != nil || !closed {
					ce = fmt.Errorf("twr: cleanup panic: %v", q)
				}
			}()
			ce = r.Close()
			closed = true
		}()
		// TinyGo recover returns nil for panic(nil), unlike current standard Go.
		if p != nil || !returned {
			panic(p)
		}
		err = errors.Join(err, ce)
	}()
	err = fn(r)
	returned = true
	return err
}

type resourceState struct {
	c       *Client
	handle  any
	release string
	closed  bool
}
type resource struct{ state *resourceState }

func own(c *Client, h any, release string) resource {
	return resource{&resourceState{c: c, handle: h, release: release}}
}
func (r resource) handle() (any, error) {
	if r.state == nil || r.state.closed {
		return nil, ErrClosed
	}
	return r.state.handle, nil
}
func (r resource) Close() error {
	if r.state == nil || r.state.closed {
		return nil
	}
	r.state.closed = true
	return r.state.c.do(r.state.release, r.state.handle)
}
func (r resource) client() *Client { return r.state.c }
func resourceCall[T any](r resource, op string, args ...any) (T, error) {
	var z T
	h, e := r.handle()
	if e != nil {
		return z, e
	}
	return call[T](r.client(), op, append([]any{h}, args...)...)
}
func resourceDo(r resource, op string, args ...any) error {
	h, e := r.handle()
	if e != nil {
		return e
	}
	return r.client().do(op, append([]any{h}, args...)...)
}
func same(c *Client, rs ...resource) error {
	for _, r := range rs {
		if _, e := r.handle(); e != nil {
			return e
		}
		if r.client() != c {
			return fmt.Errorf("twr: resources belong to different clients")
		}
	}
	return nil
}
func (c *Client) row(v any) (out []any, err error) {
	if v == nil {
		return nil, nil
	}
	h, ok := v.(types.BoxTuple)
	if !ok {
		return nil, fmt.Errorf("twr: expected tuple handle, got %T", v)
	}
	t := &Tuple{own(c, h, "box-tuple.Release")}
	err = Using(t, func(t *Tuple) error { var e error; out, e = t.Values(); return e })
	return out, err
}
func (c *Client) rowCall(op string, args ...any) ([]any, error) {
	v, e := c.host.Call(op, args...)
	if e != nil {
		return nil, e
	}
	return c.row(v)
}
