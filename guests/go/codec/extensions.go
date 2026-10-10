package codec

import (
	"encoding/binary"
	"fmt"
	"math"
	"reflect"
	"strings"
)

func encodeExtension(v any, depth int) (Extension, bool, error) {
	bad := func(s string) (Extension, bool, error) {
		return Extension{}, true, fmt.Errorf("MessagePack extension: %s", s)
	}
	switch x := v.(type) {
	case Extension:
		return x, true, nil
	case UUID:
		return Extension{2, x[:]}, true, nil
	case Decimal:
		if x.Digits == "" || len(x.Digits) > 38 {
			return bad("decimal requires 1..38 digits")
		}
		for _, c := range x.Digits {
			if c < '0' || c > '9' {
				return bad("invalid decimal digit")
			}
		}
		s := x.Digits
		if x.Negative {
			s += "d"
		} else {
			s += "c"
		}
		if len(s)%2 != 0 {
			s = "0" + s
		}
		b := integer(nil, uint64(int64(x.Scale)), x.Scale < 0)
		for i := 0; i < len(s); i += 2 {
			a := s[i] - '0'
			z := s[i+1] - '0'
			if s[i+1] == 'c' {
				z = 12
			}
			if s[i+1] == 'd' {
				z = 13
			}
			b = append(b, a<<4|z)
		}
		return Extension{1, b}, true, nil
	case Datetime:
		if x.Nanoseconds < 0 || x.Nanoseconds >= 1e9 {
			return bad("datetime nanoseconds out of range")
		}
		b := binary.LittleEndian.AppendUint64(nil, uint64(x.Seconds))
		if x.Nanoseconds != 0 || x.TZOffset != 0 || x.TZIndex != 0 {
			b = binary.LittleEndian.AppendUint32(b, uint32(x.Nanoseconds))
			b = binary.LittleEndian.AppendUint16(b, uint16(x.TZOffset))
			b = binary.LittleEndian.AppendUint16(b, uint16(x.TZIndex))
		}
		return Extension{4, b}, true, nil
	case Interval:
		if x.Adjust < 0 || x.Adjust > 2 {
			return bad("invalid interval adjust")
		}
		values := []int64{x.Years, x.Months, x.Weeks, x.Days, x.Hours, x.Minutes, x.Seconds, x.Nanoseconds, x.Adjust}
		b := []byte{0}
		for i, n := range values {
			if n != 0 {
				b[0]++
				b = integer(append(b, byte(i)), uint64(n), n < 0)
			}
		}
		return Extension{6, b}, true, nil
	case ErrorValue:
		if len(x.Stack) == 0 {
			return bad("empty error stack")
		}
		m := map[uint64]any{}
		for k, v := range x.Extra {
			if k == 0 {
				return bad("reserved error key")
			}
			m[k] = v
		}
		frames := make([]any, len(x.Stack))
		for i, f := range x.Stack {
			fm := map[uint64]any{}
			for k, v := range f.Extra {
				if k < 7 {
					return bad("reserved error frame key")
				}
				fm[k] = v
			}
			fm[0] = f.Type
			fm[1] = f.File
			fm[2] = f.Line
			fm[3] = f.Message
			fm[4] = f.Errno
			fm[5] = f.Code
			if f.Fields == nil {
				fm[6] = map[string]any{}
			} else {
				fm[6] = f.Fields
			}
			frames[i] = fm
		}
		m[0] = frames
		b, e := encode(nil, reflect.ValueOf(m), depth+1)
		return Extension{3, b}, true, e
	}
	return Extension{}, false, nil
}
func signed(v any) (int64, error) {
	switch n := v.(type) {
	case int64:
		return n, nil
	case uint64:
		if n <= math.MaxInt64 {
			return int64(n), nil
		}
	}
	return 0, fmt.Errorf("expected signed integer")
}
func numericMap(v any) (map[uint64]any, error) {
	m, ok := v.(map[any]any)
	if !ok {
		return nil, fmt.Errorf("expected numeric map")
	}
	out := map[uint64]any{}
	for k, v := range m {
		n, ok := k.(uint64)
		if !ok {
			return nil, fmt.Errorf("invalid numeric map key")
		}
		out[n] = v
	}
	return out, nil
}
func decodeExtension(x Extension, depth int) (any, error) {
	bad := func(s string) (any, error) {
		return nil, fmt.Errorf("MessagePack: invalid extension %d: %s", x.Type, s)
	}
	switch x.Type {
	case 2:
		if len(x.Data) != 16 {
			return bad("UUID length")
		}
		var u UUID
		copy(u[:], x.Data)
		return u, nil
	case 1:
		r := reader{b: x.Data}
		v, e := r.read(depth)
		if e != nil {
			return nil, e
		}
		scale, e := signed(v)
		if e != nil || scale < math.MinInt32 || scale > math.MaxInt32 {
			return bad("decimal scale")
		}
		b := x.Data[r.pos:]
		if len(b) == 0 {
			return bad("decimal digits")
		}
		sign := b[len(b)-1] & 15
		if sign < 10 || sign > 15 {
			return bad("decimal sign")
		}
		var s strings.Builder
		for i, z := range b {
			if z>>4 > 9 {
				return bad("decimal digit")
			}
			s.WriteByte('0' + (z >> 4))
			if i < len(b)-1 {
				if z&15 > 9 {
					return bad("decimal digit")
				}
				s.WriteByte('0' + (z & 15))
			}
		}
		digits := strings.TrimLeft(s.String(), "0")
		if digits == "" {
			digits = "0"
		}
		if len(digits) > 38 {
			return bad("decimal precision")
		}
		return Decimal{digits, int32(scale), sign == 11 || sign == 13}, nil
	case 4:
		b := x.Data
		if len(b) != 8 && len(b) != 16 {
			return bad("datetime length")
		}
		v := Datetime{Seconds: int64(binary.LittleEndian.Uint64(b))}
		if len(b) == 16 {
			v.Nanoseconds = int32(binary.LittleEndian.Uint32(b[8:]))
			v.TZOffset = int16(binary.LittleEndian.Uint16(b[12:]))
			v.TZIndex = int16(binary.LittleEndian.Uint16(b[14:]))
		}
		if v.Nanoseconds < 0 || v.Nanoseconds >= 1e9 {
			return bad("datetime nanoseconds")
		}
		return v, nil
	case 6:
		if len(x.Data) == 0 || x.Data[0] > 9 {
			return bad("interval count")
		}
		r := reader{b: x.Data, pos: 1}
		var values [9]int64
		var seen [9]bool
		for i := 0; i < int(x.Data[0]); i++ {
			id, e := r.number(1)
			if e != nil {
				return nil, e
			}
			if id >= 9 || seen[id] {
				return bad("interval field")
			}
			seen[id] = true
			v, e := r.read(depth)
			if e != nil {
				return nil, e
			}
			values[id], e = signed(v)
			if e != nil {
				return nil, e
			}
		}
		if r.pos != len(x.Data) || values[8] < 0 || values[8] > 2 {
			return bad("interval payload")
		}
		return Interval{values[0], values[1], values[2], values[3], values[4], values[5], values[6], values[7], values[8]}, nil
	case 3:
		r := reader{b: x.Data}
		v, e := r.read(depth)
		if e != nil {
			return nil, e
		}
		if r.pos != len(x.Data) {
			return bad("trailing error bytes")
		}
		m, e := numericMap(v)
		if e != nil {
			return bad(e.Error())
		}
		stack, ok := m[0].([]any)
		if !ok || len(stack) == 0 {
			return bad("missing error stack")
		}
		delete(m, 0)
		out := ErrorValue{Extra: m, Stack: make([]ErrorFrame, len(stack))}
		for i, v := range stack {
			fm, e := numericMap(v)
			if e != nil {
				return bad(e.Error())
			}
			f := ErrorFrame{Extra: map[uint64]any{}, Fields: map[string]any{}}
			for _, k := range []uint64{0, 1, 3} {
				s, ok := fm[k].(string)
				if !ok {
					return bad("missing error frame string")
				}
				switch k {
				case 0:
					f.Type = s
				case 1:
					f.File = s
				case 3:
					f.Message = s
				}
			}
			for _, k := range []uint64{2, 4, 5} {
				if v, ok := fm[k]; ok {
					n, e := signed(v)
					if e != nil || n < 0 || n > math.MaxUint32 {
						return bad("error frame integer")
					}
					switch k {
					case 2:
						f.Line = uint32(n)
					case 4:
						f.Errno = uint32(n)
					case 5:
						f.Code = uint32(n)
					}
				}
			}
			if v, ok := fm[6]; ok {
				fields, ok := v.(map[string]any)
				if !ok {
					return bad("error fields map")
				}
				f.Fields = fields
			}
			for k, v := range fm {
				if k >= 7 {
					f.Extra[k] = v
				}
			}
			out.Stack[i] = f
		}
		return out, nil
	}
	return x, nil
}
