package twr

import (
	"fmt"
	"reflect"

	"github.com/tarantool/tarantool-wasm-rs/guests/go/bindings"
	"github.com/tarantool/tarantool-wasm-rs/guests/go/codec"
	"github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/types"
	"go.bytecodealliance.org/cm"
)

func toOption[T any](p *T) cm.Option[T] {
	if p == nil {
		return cm.None[T]()
	}
	return cm.Some(*p)
}
func fromOption[T any](o cm.Option[T]) *T {
	if p := o.Some(); p != nil {
		v := *p
		return &v
	}
	return nil
}

// Sequence addresses existing schema; the WIT API cannot create sequences.
type Sequence struct {
	c  *Client
	ID uint32
}

func (c *Client) Sequence(id uint32) *Sequence { return &Sequence{c, id} }
func (s *Sequence) Next() (int64, error) {
	return call[int64](s.c, "sequence.Next", types.Sequence{ID: s.ID})
}
func (s *Sequence) Current() (int64, error) {
	return call[int64](s.c, "sequence.Current", types.Sequence{ID: s.ID})
}
func (s *Sequence) Set(v int64) error { return s.c.do("sequence.Set", types.Sequence{ID: s.ID}, v) }
func (s *Sequence) Reset() error      { return s.c.do("sequence.Reset", types.Sequence{ID: s.ID}) }

// Session is a borrowed catalog ID; it does not own a connection.
type Session struct {
	c  *Client
	ID uint64
}

func (c *Client) Session(id uint64) *Session { return &Session{c, id} }
func (c *Client) CurrentSession() (*Session, error) {
	s, e := call[types.Session](c, "session.Current")
	if e != nil {
		return nil, e
	}
	return c.Session(s.ID), nil
}
func mapBytes(v any) ([]byte, error) {
	if v == nil {
		return nil, fmt.Errorf("twr: expected MessagePack map")
	}
	if _, ok := v.(codec.Pairs); !ok && reflect.TypeOf(v).Kind() != reflect.Map {
		return nil, fmt.Errorf("twr: expected MessagePack map")
	}
	b, e := Pack(v)
	if e == nil && b[0] == 0xc0 {
		e = fmt.Errorf("twr: expected non-nil map")
	}
	return b, e
}
func (s *Session) Send(header, body any) error {
	hb, e := mapBytes(header)
	if e != nil {
		return e
	}
	var bb *types.MsgpackValue
	if body != nil {
		b, e := mapBytes(body)
		if e != nil {
			return e
		}
		v := types.MsgpackValue(cm.ToList(b))
		bb = &v
	}
	return s.c.do("session.IprotoSend", types.Session{ID: s.ID}, hb, bb)
}

// BroadcastLua forwards a Lua expression as required by WIT. It is not a value codec.
func (c *Client) BroadcastLua(key, expression string) error {
	return c.do("session.Broadcast", key, expression)
}
func (c *Client) NewError(code uint32, message string) (*Error, error) {
	v, e := call[types.BoxError](c, "error.New", code, message)
	if e != nil {
		return nil, e
	}
	return bindings.FromRecord(v), nil
}
func (c *Client) NewErrorWithLocation(code uint32, message string, file *string, line *uint32) (*Error, error) {
	v, e := call[types.BoxError](c, "error.NewWithLocation", code, message, file, line)
	if e != nil {
		return nil, e
	}
	return bindings.FromRecord(v), nil
}
func errorRecord(e *Error) types.BoxError {
	return types.BoxError{Code: types.ErrorCode(e.Code), Message: e.Message, ErrorType: e.Type, File: toOption(e.File), Line: toOption(e.Line)}
}
func (c *Client) SetError(e *Error) error {
	if e == nil {
		return fmt.Errorf("twr: nil error record")
	}
	return c.do("error.Set", errorRecord(e))
}
func (c *Client) LastError() (*Error, error) {
	v, e := c.host.Call("error.Last")
	if e != nil {
		return nil, e
	}
	if v == nil {
		return nil, nil
	}
	r, ok := v.(types.BoxError)
	if !ok {
		return nil, fmt.Errorf("twr: invalid error binding")
	}
	return bindings.FromRecord(r), nil
}
func (c *Client) ClearError() error { return c.do("error.Clear") }
func (c *Client) FormatError(e *Error) (string, error) {
	if e == nil {
		return "", fmt.Errorf("twr: nil error record")
	}
	return call[string](c, "error.ToString", errorRecord(e))
}

type LogContext struct {
	File string
	Line uint32
}

func (c *Client) Log(level LogLevel, message string, context *LogContext) error {
	if level > LogLevelDebug {
		return fmt.Errorf("twr: invalid log level %d", level)
	}
	var raw *types.LogContext
	if context != nil {
		raw = &types.LogContext{File: context.File, Line: context.Line}
	}
	return c.do("log.Write", level, message, raw)
}
func (c *Client) Uptime() (float64, error) { return call[float64](c, "metrics.Uptime") }

// FromJSON and ToJSON explicitly expose the WIT conversion functions. Ordinary
// SDK operations use the value codec directly, without JSON intermediates.
func (c *Client) FromJSON(s string) ([]byte, error) { return call[[]byte](c, "msgpack.FromJSON", s) }
func (c *Client) ToJSON(b []byte) (string, error)   { return call[string](c, "msgpack.ToJSON", b) }
