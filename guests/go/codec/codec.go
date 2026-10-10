package codec

import (
	"encoding/binary"
	"fmt"
	"math"
	"reflect"
	"unicode/utf8"
)

const maxDepth = 128

// Pack accepts booleans, integers, floats, strings, byte slices, arrays, maps,
// Pairs, pointers, and the extension types in this package. Typed nil is nil.
// Arbitrary structs, functions, channels and cyclic structures are rejected.
func Pack(v any) ([]byte, error) { return encode(nil, reflect.ValueOf(v), 0) }
func head(b []byte, n int, fix byte, cap int, t8, t16, t32 byte) []byte {
	if n < cap {
		return append(b, fix|byte(n))
	}
	if n < 256 && t8 != 0 {
		return append(b, t8, byte(n))
	}
	if n < 65536 {
		return binary.BigEndian.AppendUint16(append(b, t16), uint16(n))
	}
	return binary.BigEndian.AppendUint32(append(b, t32), uint32(n))
}
func integer(b []byte, v uint64, negative bool) []byte {
	if !negative {
		switch {
		case v < 128:
			return append(b, byte(v))
		case v <= 255:
			return append(b, 0xcc, byte(v))
		case v <= 65535:
			return binary.BigEndian.AppendUint16(append(b, 0xcd), uint16(v))
		case v <= math.MaxUint32:
			return binary.BigEndian.AppendUint32(append(b, 0xce), uint32(v))
		default:
			return binary.BigEndian.AppendUint64(append(b, 0xcf), v)
		}
	}
	n := int64(v)
	switch {
	case n >= -32:
		return append(b, byte(n))
	case n >= -128:
		return append(b, 0xd0, byte(n))
	case n >= -32768:
		return binary.BigEndian.AppendUint16(append(b, 0xd1), uint16(n))
	case n >= math.MinInt32:
		return binary.BigEndian.AppendUint32(append(b, 0xd2), uint32(n))
	default:
		return binary.BigEndian.AppendUint64(append(b, 0xd3), v)
	}
}
func encode(b []byte, v reflect.Value, depth int) ([]byte, error) {
	if depth > maxDepth {
		return nil, fmt.Errorf("MessagePack: cyclic value or nesting exceeds %d", maxDepth)
	}
	if !v.IsValid() {
		return append(b, 0xc0), nil
	}
	switch v.Kind() {
	case reflect.Interface, reflect.Pointer:
		if v.IsNil() {
			return append(b, 0xc0), nil
		}
		return encode(b, v.Elem(), depth+1)
	case reflect.Slice, reflect.Map:
		if v.IsNil() {
			return append(b, 0xc0), nil
		}
	}
	if v.CanInterface() {
		if pairs, ok := v.Interface().(Pairs); ok {
			b = head(b, len(pairs), 0x80, 16, 0, 0xde, 0xdf)
			var e error
			for _, p := range pairs {
				b, e = encode(b, reflect.ValueOf(p.Key), depth+1)
				if e != nil {
					return nil, e
				}
				b, e = encode(b, reflect.ValueOf(p.Value), depth+1)
				if e != nil {
					return nil, e
				}
			}
			return b, nil
		}
		x, ok, e := encodeExtension(v.Interface(), depth)
		if e != nil {
			return nil, e
		}
		if ok {
			return appendExtension(b, x), nil
		}
	}
	switch v.Kind() {
	case reflect.Bool:
		if v.Bool() {
			return append(b, 0xc3), nil
		}
		return append(b, 0xc2), nil
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		n := v.Int()
		return integer(b, uint64(n), n < 0), nil
	case reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64:
		return integer(b, v.Uint(), false), nil
	case reflect.Float32:
		return binary.BigEndian.AppendUint32(append(b, 0xca), math.Float32bits(float32(v.Float()))), nil
	case reflect.Float64:
		return binary.BigEndian.AppendUint64(append(b, 0xcb), math.Float64bits(v.Float())), nil
	case reflect.String:
		s := v.String()
		if !utf8.ValidString(s) {
			return nil, fmt.Errorf("MessagePack: invalid UTF-8 string")
		}
		return append(head(b, len(s), 0xa0, 32, 0xd9, 0xda, 0xdb), s...), nil
	case reflect.Slice, reflect.Array:
		if v.Kind() == reflect.Slice && v.Type().Elem().Kind() == reflect.Uint8 {
			data := v.Bytes()
			return append(head(b, len(data), 0, 0, 0xc4, 0xc5, 0xc6), data...), nil
		}
		b = head(b, v.Len(), 0x90, 16, 0, 0xdc, 0xdd)
		var e error
		for i := 0; i < v.Len(); i++ {
			b, e = encode(b, v.Index(i), depth+1)
			if e != nil {
				return nil, e
			}
		}
		return b, nil
	case reflect.Map:
		b = head(b, v.Len(), 0x80, 16, 0, 0xde, 0xdf)
		it := v.MapRange()
		var e error
		for it.Next() {
			b, e = encode(b, it.Key(), depth+1)
			if e != nil {
				return nil, e
			}
			b, e = encode(b, it.Value(), depth+1)
			if e != nil {
				return nil, e
			}
		}
		return b, nil
	}
	return nil, fmt.Errorf("MessagePack: unsupported Go type %s", v.Type())
}
func appendExtension(b []byte, x Extension) []byte {
	switch len(x.Data) {
	case 1:
		b = append(b, 0xd4)
	case 2:
		b = append(b, 0xd5)
	case 4:
		b = append(b, 0xd6)
	case 8:
		b = append(b, 0xd7)
	case 16:
		b = append(b, 0xd8)
	default:
		b = head(b, len(x.Data), 0, 0, 0xc7, 0xc8, 0xc9)
	}
	return append(append(b, byte(x.Type)), x.Data...)
}

type reader struct {
	b   []byte
	pos int
}

func (r *reader) take(n int) ([]byte, error) {
	if n < 0 || n > len(r.b)-r.pos {
		return nil, fmt.Errorf("MessagePack: truncated value at byte %d", r.pos)
	}
	b := r.b[r.pos : r.pos+n]
	r.pos += n
	return b, nil
}
func (r *reader) number(n int) (uint64, error) {
	b, e := r.take(n)
	if e != nil {
		return 0, e
	}
	switch n {
	case 1:
		return uint64(b[0]), nil
	case 2:
		return uint64(binary.BigEndian.Uint16(b)), nil
	case 4:
		return uint64(binary.BigEndian.Uint32(b)), nil
	case 8:
		return binary.BigEndian.Uint64(b), nil
	}
	panic("invalid number width")
}

// Unpack decodes exactly one value. All unsigned integers decode as uint64;
// signed encodings decode as int64. Empty containers are non-nil.
func Unpack(b []byte) (any, error) {
	r := reader{b: b}
	v, e := r.read(0)
	if e != nil {
		return nil, e
	}
	if r.pos != len(b) {
		return nil, fmt.Errorf("MessagePack: trailing bytes at %d", r.pos)
	}
	return v, nil
}
func (r *reader) read(depth int) (any, error) {
	if depth > maxDepth {
		return nil, fmt.Errorf("MessagePack: nesting exceeds %d", maxDepth)
	}
	t, e := r.number(1)
	if e != nil {
		return nil, e
	}
	tag := byte(t)
	if tag < 128 {
		return uint64(tag), nil
	}
	if tag >= 224 {
		return int64(int8(tag)), nil
	}
	if tag&0xe0 == 0xa0 {
		return r.str(int(tag & 31))
	}
	if tag&0xf0 == 0x90 {
		return r.array(int(tag&15), depth)
	}
	if tag&0xf0 == 0x80 {
		return r.mapping(int(tag&15), depth)
	}
	switch tag {
	case 0xc0:
		return nil, nil
	case 0xc2, 0xc3:
		return tag == 0xc3, nil
	case 0xcc, 0xcd, 0xce, 0xcf:
		return r.number(1 << uint(tag-0xcc))
	case 0xd0, 0xd1, 0xd2, 0xd3:
		n := 1 << uint(tag-0xd0)
		v, e := r.number(n)
		if e != nil {
			return nil, e
		}
		switch n {
		case 1:
			return int64(int8(v)), nil
		case 2:
			return int64(int16(v)), nil
		case 4:
			return int64(int32(v)), nil
		default:
			return int64(v), nil
		}
	case 0xca:
		v, e := r.number(4)
		return float64(math.Float32frombits(uint32(v))), e
	case 0xcb:
		v, e := r.number(8)
		return math.Float64frombits(v), e
	case 0xc4, 0xc5, 0xc6:
		n, e := r.number(1 << uint(tag-0xc4))
		if e != nil {
			return nil, e
		}
		b, e := r.take(int(n))
		if e != nil {
			return nil, e
		}
		return append([]byte{}, b...), nil
	case 0xd9, 0xda, 0xdb:
		n, e := r.number(1 << uint(tag-0xd9))
		if e != nil {
			return nil, e
		}
		return r.str(int(n))
	case 0xdc, 0xdd:
		n, e := r.number(2 << uint(tag-0xdc))
		if e != nil {
			return nil, e
		}
		return r.array(int(n), depth)
	case 0xde, 0xdf:
		n, e := r.number(2 << uint(tag-0xde))
		if e != nil {
			return nil, e
		}
		return r.mapping(int(n), depth)
	case 0xc7, 0xc8, 0xc9, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8:
		var n uint64
		if tag >= 0xd4 {
			n = 1 << uint(tag-0xd4)
		} else {
			n, e = r.number(1 << uint(tag-0xc7))
			if e != nil {
				return nil, e
			}
		}
		code, e := r.number(1)
		if e != nil {
			return nil, e
		}
		b, e := r.take(int(n))
		if e != nil {
			return nil, e
		}
		return decodeExtension(Extension{int8(code), append([]byte{}, b...)}, depth+1)
	}
	return nil, fmt.Errorf("MessagePack: unsupported tag 0x%02x", tag)
}
func (r *reader) str(n int) (any, error) {
	b, e := r.take(n)
	if e != nil {
		return nil, e
	}
	if !utf8.Valid(b) {
		return nil, fmt.Errorf("MessagePack: invalid UTF-8")
	}
	return string(b), nil
}
func (r *reader) array(n, depth int) (any, error) {
	if n < 0 || n > len(r.b)-r.pos {
		return nil, fmt.Errorf("MessagePack: truncated array")
	}
	a := make([]any, n)
	for i := range a {
		v, e := r.read(depth + 1)
		if e != nil {
			return nil, e
		}
		a[i] = v
	}
	return a, nil
}
func (r *reader) mapping(n, depth int) (any, error) {
	if n < 0 || n > (len(r.b)-r.pos)/2 {
		return nil, fmt.Errorf("MessagePack: truncated map")
	}
	pairs := make(Pairs, n)
	strings := true
	comparable := true
	for i := range pairs {
		k, e := r.read(depth + 1)
		if e != nil {
			return nil, e
		}
		v, e := r.read(depth + 1)
		if e != nil {
			return nil, e
		}
		pairs[i] = Pair{k, v}
		if _, ok := k.(string); !ok {
			strings = false
		}
		if k != nil && !reflect.TypeOf(k).Comparable() {
			comparable = false
		}
		if f, ok := k.(float64); ok && math.IsNaN(f) {
			comparable = false
		}
	}
	if comparable {
		m := make(map[any]any, n)
		for _, p := range pairs {
			if _, dup := m[p.Key]; dup {
				return pairs, nil
			}
			m[p.Key] = p.Value
		}
		if strings {
			sm := make(map[string]any, n)
			for k, v := range m {
				sm[k.(string)] = v
			}
			return sm, nil
		}
		return m, nil
	}
	return pairs, nil
}
