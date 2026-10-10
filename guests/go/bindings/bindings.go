// Package bindings isolates generated WIT bindings from the public API.
package bindings

import (
	"fmt"

	"github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/types"
)

// Caller permits deterministic unit tests. Arguments use raw records, Go slices
// and pointers for optional parameters. Results use nil for absent WIT options.
type Caller interface {
	Call(operation string, args ...any) (any, error)
}

// Error preserves every field in the WIT box-error record.
type Error struct {
	Code    uint32
	Message string
	Type    string
	File    *string
	Line    *uint32
}

func (e *Error) Error() string { return fmt.Sprintf("%s (%d): %s", e.Type, e.Code, e.Message) }

// Is matches the numeric code and, when specified, the host error type.
func (e *Error) Is(target error) bool {
	t, ok := target.(*Error)
	return ok && e.Code == t.Code && (t.Type == "" || e.Type == t.Type)
}
func FromRecord(r types.BoxError) *Error {
	e := &Error{Code: uint32(r.Code), Message: r.Message, Type: r.ErrorType}
	if p := r.File.Some(); p != nil {
		v := *p
		e.File = &v
	}
	if p := r.Line.Some(); p != nil {
		v := *p
		e.Line = &v
	}
	return e
}
