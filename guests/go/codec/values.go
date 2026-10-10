// Package codec encodes Go values directly as MessagePack, including Tarantool extensions.
package codec

// Pair represents a map entry whose key need not be comparable in Go.
type Pair struct{ Key, Value any }

// Pairs preserves map order, duplicate keys, and unrepresentable Go map keys.
type Pairs []Pair

// Extension preserves an unknown MessagePack extension verbatim.
type Extension struct {
	Type int8
	Data []byte
}

// UUID is the 16-byte Tarantool UUID extension.
type UUID [16]byte

// Decimal is an exact BCD decimal: Digits * 10^-Scale, with an explicit sign.
type Decimal struct {
	Digits   string
	Scale    int32
	Negative bool
}

// Datetime preserves Tarantool's complete datetime payload without time.Time conversion.
type Datetime struct {
	Seconds           int64
	Nanoseconds       int32
	TZOffset, TZIndex int16
}

// Interval contains signed Tarantool interval fields. Adjust: 0 excess, 1 none, 2 last.
type Interval struct{ Years, Months, Weeks, Days, Hours, Minutes, Seconds, Nanoseconds, Adjust int64 }

// ErrorFrame is one frame of a Tarantool MessagePack error stack.
type ErrorFrame struct {
	Type, File, Message string
	Line, Errno, Code   uint32
	Fields              map[string]any
	Extra               map[uint64]any
}

// ErrorValue preserves an error stack and future numeric attributes.
type ErrorValue struct {
	Stack []ErrorFrame
	Extra map[uint64]any
}
