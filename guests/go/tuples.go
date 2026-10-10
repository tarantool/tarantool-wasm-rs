package twr

import (
	"errors"
	"fmt"

	"github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/types"
)

// Tuple owns one reference to a host tuple. Copy acquires a distinct reference;
// copying the Go value shares its closed state and does not acquire ownership.
type Tuple struct{ resource }

func (c *Client) NewTuple(v []any) (*Tuple, error) {
	b, e := array(v)
	if e != nil {
		return nil, e
	}
	h, e := call[types.BoxTuple](c, "box-tuple.New", b)
	if e != nil {
		return nil, e
	}
	return &Tuple{own(c, h, "box-tuple.Release")}, nil
}
func (t *Tuple) Copy() (*Tuple, error) {
	h, e := t.handle()
	if e != nil {
		return nil, e
	}
	if e = t.client().do("box-tuple.Retain", h); e != nil {
		return nil, e
	}
	return &Tuple{own(t.client(), h, "box-tuple.Release")}, nil
}
func (t *Tuple) FieldCount() (uint32, error) {
	return resourceCall[uint32](t.resource, "box-tuple.FieldCount")
}
func (t *Tuple) ByteSize() (uint64, error) {
	return resourceCall[uint64](t.resource, "box-tuple.ByteSize")
}
func (t *Tuple) Bytes() ([]byte, error) { return resourceCall[[]byte](t.resource, "box-tuple.ToBytes") }
func (t *Tuple) Values() ([]any, error) {
	b, e := t.Bytes()
	if e != nil {
		return nil, e
	}
	v, e := Unpack(b)
	if e != nil {
		return nil, e
	}
	a, ok := v.([]any)
	if !ok {
		return nil, fmt.Errorf("twr: host tuple is not an array")
	}
	return a, nil
}
func unpackOptional(v any, e error) (any, bool, error) {
	if e != nil {
		return nil, false, e
	}
	if v == nil {
		return nil, false, nil
	}
	var b []byte
	switch x := v.(type) {
	case []byte:
		b = x
	case types.MsgpackValue:
		b = x.Slice()
	default:
		return nil, false, fmt.Errorf("twr: binding returned %T instead of MessagePack", v)
	}
	out, e := Unpack(b)
	return out, true, e
}

// Field and FieldByPath return present=false for absence, distinguishing it from nil.
func (t *Tuple) Field(n uint32) (any, bool, error) {
	h, e := t.handle()
	if e != nil {
		return nil, false, e
	}
	return unpackOptional(t.client().host.Call("box-tuple.Field", h, n))
}
func (t *Tuple) FieldByPath(path string) (any, bool, error) {
	h, e := t.handle()
	if e != nil {
		return nil, false, e
	}
	return unpackOptional(t.client().host.Call("box-tuple.FieldByPath", h, path))
}
func (t *Tuple) Format() (*TupleFormat, error) {
	h, e := resourceCall[types.TupleFormat](t.resource, "box-tuple.Format")
	if e != nil {
		return nil, e
	}
	return &TupleFormat{own(t.client(), h, "tuple-format.Release")}, nil
}
func (t *Tuple) Update(ops []Operation) (*Tuple, error) { return t.modify("box-tuple.Update", ops) }
func (t *Tuple) Upsert(ops []Operation) (*Tuple, error) { return t.modify("box-tuple.Upsert", ops) }
func (t *Tuple) modify(op string, ops []Operation) (*Tuple, error) {
	b, e := operations(ops, true)
	if e != nil {
		return nil, e
	}
	h, e := resourceCall[types.BoxTuple](t.resource, op, b)
	if e != nil {
		return nil, e
	}
	return &Tuple{own(t.client(), h, "box-tuple.Release")}, nil
}
func (t *Tuple) Validate(f *TupleFormat) error {
	if f == nil {
		return fmt.Errorf("twr: nil tuple format")
	}
	if _, e := t.handle(); e != nil {
		return e
	}
	if e := same(t.client(), f.resource); e != nil {
		return e
	}
	h, _ := f.handle()
	return resourceDo(t.resource, "box-tuple.Validate", h)
}
func (t *Tuple) Compare(other *Tuple, k *KeyDef) (int32, error) {
	if other == nil || k == nil {
		return 0, fmt.Errorf("twr: nil comparison resource")
	}
	if _, e := t.handle(); e != nil {
		return 0, e
	}
	if e := same(t.client(), other.resource, k.resource); e != nil {
		return 0, e
	}
	kh, _ := k.handle()
	th, _ := t.handle()
	oh, _ := other.handle()
	return call[int32](t.client(), "box-tuple.Compare", kh, th, oh)
}
func (t *Tuple) CompareWithKey(v any, k *KeyDef) (int32, error) {
	if k == nil {
		return 0, fmt.Errorf("twr: nil key definition")
	}
	if _, e := t.handle(); e != nil {
		return 0, e
	}
	if e := same(t.client(), k.resource); e != nil {
		return 0, e
	}
	b, e := key(v)
	if e != nil {
		return 0, e
	}
	h, _ := k.handle()
	return resourceCall[int32](t.resource, "box-tuple.CompareWithKey", b, h)
}

// TupleCursor owns an independent host reference to its tuple. Read allows
// rewind after exhaustion; Next closes on exhaustion. Both close on errors.
type TupleCursor struct {
	resource
	exhausted bool
}

func (t *Tuple) Cursor() (*TupleCursor, error) {
	h, e := resourceCall[any](t.resource, "box-tuple.IteratorNew")
	if e != nil {
		return nil, e
	}
	return &TupleCursor{resource: own(t.client(), h, "box-tuple.iterator.ResourceDrop")}, nil
}
func (it *TupleCursor) Read() (v any, present bool, err error) {
	h, e := it.handle()
	if e != nil {
		return nil, false, e
	}
	v, present, err = unpackOptional(it.client().host.Call("box-tuple.iterator.Next", h))
	if err != nil {
		err = errors.Join(err, it.Close())
	}
	return
}
func (it *TupleCursor) Next() (any, bool, error) {
	if it.exhausted {
		return nil, false, nil
	}
	v, ok, e := it.Read()
	if e != nil {
		return nil, false, e
	}
	if !ok {
		it.exhausted = true
		e = it.Close()
	}
	return v, ok, e
}
func (it *TupleCursor) Position() (uint32, error) {
	return resourceCall[uint32](it.resource, "box-tuple.iterator.Position")
}
func (it *TupleCursor) Rewind() error { return resourceDo(it.resource, "box-tuple.iterator.Rewind") }
func (it *TupleCursor) Seek(n uint32) (any, bool, error) {
	h, e := it.handle()
	if e != nil {
		return nil, false, e
	}
	v, ok, e := unpackOptional(it.client().host.Call("box-tuple.iterator.Seek", h, n))
	if e != nil {
		e = errors.Join(e, it.Close())
	}
	return v, ok, e
}
func (t *Tuple) Each(fn func(any) (bool, error)) error {
	it, e := t.Cursor()
	if e != nil {
		return e
	}
	return Using(it, func(it *TupleCursor) error {
		for {
			v, ok, e := it.Next()
			if e != nil {
				return e
			}
			if !ok {
				return nil
			}
			more, e := fn(v)
			if e != nil {
				return e
			}
			if !more {
				return nil
			}
		}
	})
}

// TupleFormat owns one retained format reference.
type TupleFormat struct{ resource }

func (c *Client) DefaultTupleFormat() (*TupleFormat, error) {
	h, e := call[types.TupleFormat](c, "tuple-format.Default")
	if e != nil {
		return nil, e
	}
	return &TupleFormat{own(c, h, "tuple-format.Release")}, nil
}
func (c *Client) NewTupleFormat(defs ...*KeyDef) (*TupleFormat, error) {
	hs := make([]types.KeyDef, len(defs))
	for i, d := range defs {
		if d == nil {
			return nil, fmt.Errorf("twr: nil key definition at position %d", i)
		}
		if e := same(c, d.resource); e != nil {
			return nil, e
		}
		h, _ := d.handle()
		hs[i] = h.(types.KeyDef)
	}
	h, e := call[types.TupleFormat](c, "tuple-format.New", hs)
	if e != nil {
		return nil, e
	}
	return &TupleFormat{own(c, h, "tuple-format.Release")}, nil
}
func (f *TupleFormat) Copy() (*TupleFormat, error) {
	h, e := f.handle()
	if e != nil {
		return nil, e
	}
	if e = f.client().do("tuple-format.Retain", h); e != nil {
		return nil, e
	}
	return &TupleFormat{own(f.client(), h, "tuple-format.Release")}, nil
}

// KeyPart uses zero-based field numbers and ordinary optional pointers.
type KeyPart struct {
	FieldNo   uint32
	FieldType FieldType
	Collation *Collation
	Path      *string
	Flags     KeyPartFlags
}

// KeyDef owns a host key definition.
type KeyDef struct{ resource }

func (c *Client) NewKeyDef(parts []KeyPart) (*KeyDef, error) {
	ps := make([]types.KeyPart, len(parts))
	for i, p := range parts {
		if p.FieldType > FieldTypeScalar || p.Collation != nil && *p.Collation > CollationUnicodeZhZhuyinS3 || p.Flags & ^(KeyPartFlagsIsNullable|KeyPartFlagsExcludeNull) != 0 {
			return nil, fmt.Errorf("twr: invalid key part at position %d", i)
		}
		ps[i] = types.KeyPart{FieldNo: p.FieldNo, FieldType: p.FieldType, Collation: toOption(p.Collation), Path: toOption(p.Path), Flags: p.Flags}
	}
	h, e := call[types.KeyDef](c, "key-def.New", ps)
	if e != nil {
		return nil, e
	}
	return &KeyDef{own(c, h, "key-def.Release")}, nil
}
func (k *KeyDef) Copy() (*KeyDef, error) {
	h, e := resourceCall[types.KeyDef](k.resource, "key-def.Duplicate")
	if e != nil {
		return nil, e
	}
	return &KeyDef{own(k.client(), h, "key-def.Release")}, nil
}
func (k *KeyDef) Merge(other *KeyDef) (*KeyDef, error) {
	if other == nil {
		return nil, fmt.Errorf("twr: nil key definition")
	}
	if _, e := k.handle(); e != nil {
		return nil, e
	}
	if e := same(k.client(), other.resource); e != nil {
		return nil, e
	}
	oh, _ := other.handle()
	h, e := resourceCall[types.KeyDef](k.resource, "key-def.Merge", oh)
	if e != nil {
		return nil, e
	}
	return &KeyDef{own(k.client(), h, "key-def.Release")}, nil
}
func (k *KeyDef) PartCount() (uint32, error) {
	return resourceCall[uint32](k.resource, "key-def.PartCount")
}
func (k *KeyDef) Parts() ([]KeyPart, error) {
	ps, e := resourceCall[[]types.KeyPart](k.resource, "key-def.DumpParts")
	if e != nil {
		return nil, e
	}
	out := make([]KeyPart, len(ps))
	for i, p := range ps {
		out[i] = KeyPart{p.FieldNo, p.FieldType, fromOption(p.Collation), fromOption(p.Path), p.Flags}
	}
	return out, nil
}
func (k *KeyDef) ValidateKey(v any, full bool) error {
	b, e := key(v)
	if e != nil {
		return e
	}
	op := "key-def.ValidateKey"
	if full {
		op = "key-def.ValidateFullKey"
	}
	return resourceDo(k.resource, op, b)
}
func (k *KeyDef) ExtractKey(t *Tuple) ([]any, error) {
	if t == nil {
		return nil, fmt.Errorf("twr: nil tuple")
	}
	if _, e := k.handle(); e != nil {
		return nil, e
	}
	if e := same(k.client(), t.resource); e != nil {
		return nil, e
	}
	h, _ := t.handle()
	b, e := resourceCall[[]byte](k.resource, "key-def.ExtractKey", h)
	if e != nil {
		return nil, e
	}
	v, e := Unpack(b)
	if e != nil {
		return nil, e
	}
	a, ok := v.([]any)
	if !ok {
		return nil, fmt.Errorf("twr: host key is not an array")
	}
	return a, nil
}
func (k *KeyDef) ValidateTuple(t *Tuple) error {
	if t == nil {
		return fmt.Errorf("twr: nil tuple")
	}
	if _, e := k.handle(); e != nil {
		return e
	}
	if e := same(k.client(), t.resource); e != nil {
		return e
	}
	h, _ := t.handle()
	return resourceDo(k.resource, "key-def.ValidateTuple", h)
}
