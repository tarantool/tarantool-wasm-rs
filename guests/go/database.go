package twr

import (
	"errors"
	"fmt"
	"math"

	"github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/types"
)

// Space addresses schema by name or uint32 ID. Names resolve on every operation.
type Space struct {
	c        *Client
	nameOrID any
}

func (c *Client) Space(nameOrID any) *Space { return &Space{c, nameOrID} }
func identifier(v any) (uint32, bool, error) {
	switch n := v.(type) {
	case string:
		return 0, false, nil
	case uint32:
		return n, true, nil
	case uint64:
		if n <= math.MaxUint32 {
			return uint32(n), true, nil
		}
	case int:
		if n >= 0 && uint64(n) <= math.MaxUint32 {
			return uint32(n), true, nil
		}
	}
	return 0, false, fmt.Errorf("twr: expected name or uint32 ID, got %v", v)
}
func (s *Space) resolve() (types.Space, error) {
	id, numeric, e := identifier(s.nameOrID)
	if e != nil {
		return types.Space{}, e
	}
	if numeric {
		return types.Space{ID: id}, nil
	}
	v, e := s.c.host.Call("database.SpaceByName", s.nameOrID)
	if e != nil {
		return types.Space{}, e
	}
	if v == nil {
		return types.Space{}, fmt.Errorf("%w: space %q", ErrNotFound, s.nameOrID)
	}
	r, ok := v.(types.Space)
	if !ok {
		return types.Space{}, fmt.Errorf("twr: invalid space binding")
	}
	return r, nil
}
func (s *Space) ID() (uint32, error)            { r, e := s.resolve(); return r.ID, e }
func (s *Space) Index(nameOrID any) *Index      { return &Index{s, nameOrID} }
func (s *Space) Insert(v []any) ([]any, error)  { return s.write("database.Insert", v) }
func (s *Space) Replace(v []any) ([]any, error) { return s.write("database.Replace", v) }
func (s *Space) write(op string, v []any) ([]any, error) {
	b, e := array(v)
	if e != nil {
		return nil, e
	}
	r, e := s.resolve()
	if e != nil {
		return nil, e
	}
	return s.c.rowCall(op, r, b)
}
func (s *Space) Truncate() error {
	r, e := s.resolve()
	if e != nil {
		return e
	}
	return s.c.do("database.Truncate", r)
}
func (s *Space) Get(k any) ([]any, error)    { return s.Index(uint32(0)).Get(k) }
func (s *Space) Delete(k any) ([]any, error) { return s.Index(uint32(0)).Delete(k) }
func (s *Space) Update(k any, ops []Operation) ([]any, error) {
	return s.Index(uint32(0)).Update(k, ops)
}
func (s *Space) Upsert(v []any, ops []Operation) error        { return s.Index(uint32(0)).Upsert(v, ops) }
func (s *Space) Select(k any, o ScanOptions) ([][]any, error) { return s.Index(uint32(0)).Select(k, o) }
func (s *Space) Iter(k any, o ScanOptions) (*IndexIterator, error) {
	return s.Index(uint32(0)).Iter(k, o)
}
func (s *Space) Each(k any, o ScanOptions, fn func([]any) (bool, error)) error {
	return s.Index(uint32(0)).Each(k, o, fn)
}

// Index follows its parent space's current name resolution.
type Index struct {
	space    *Space
	nameOrID any
}

func (i *Index) resolve() (types.Index, error) {
	s, e := i.space.resolve()
	if e != nil {
		return types.Index{}, e
	}
	id, numeric, e := identifier(i.nameOrID)
	if e != nil {
		return types.Index{}, e
	}
	if numeric {
		return types.Index{ID: id, SpaceID: s.ID}, nil
	}
	v, e := i.space.c.host.Call("database.IndexByName", s, i.nameOrID)
	if e != nil {
		return types.Index{}, e
	}
	if v == nil {
		return types.Index{}, fmt.Errorf("%w: index %q", ErrNotFound, i.nameOrID)
	}
	r, ok := v.(types.Index)
	if !ok {
		return types.Index{}, fmt.Errorf("twr: invalid index binding")
	}
	return r, nil
}
func (i *Index) ID() (uint32, error) { r, e := i.resolve(); return r.ID, e }
func (i *Index) TupleCount() (uint64, error) {
	r, e := i.resolve()
	if e != nil {
		return 0, e
	}
	return call[uint64](i.space.c, "index.TupleCount", r)
}
func (i *Index) MemorySize() (uint64, error) {
	r, e := i.resolve()
	if e != nil {
		return 0, e
	}
	return call[uint64](i.space.c, "index.MemorySize", r)
}
func (i *Index) Get(k any) ([]any, error) { return i.read("index.Get", k) }
func (i *Index) Min(k any) ([]any, error) { return i.read("index.Min", k) }
func (i *Index) Max(k any) ([]any, error) { return i.read("index.Max", k) }
func (i *Index) read(op string, k any) ([]any, error) {
	b, e := key(k)
	if e != nil {
		return nil, e
	}
	r, e := i.resolve()
	if e != nil {
		return nil, e
	}
	return i.space.c.rowCall(op, r, b)
}
func (i *Index) Random(seed uint32) ([]any, error) {
	r, e := i.resolve()
	if e != nil {
		return nil, e
	}
	return i.space.c.rowCall("index.Random", r, seed)
}
func (i *Index) Count(k any, mode IteratorType) (uint64, error) {
	if mode > IteratorTypePreviousPrefix {
		return 0, fmt.Errorf("twr: invalid iterator type %d", mode)
	}
	b, e := key(k)
	if e != nil {
		return 0, e
	}
	r, e := i.resolve()
	if e != nil {
		return 0, e
	}
	return call[uint64](i.space.c, "index.Count", r, mode, b)
}
func (i *Index) Delete(k any) ([]any, error) { return i.read("database.Delete", k) }
func (i *Index) Update(k any, ops []Operation) ([]any, error) {
	b, e := key(k)
	if e != nil {
		return nil, e
	}
	ob, e := operations(ops, false)
	if e != nil {
		return nil, e
	}
	r, e := i.resolve()
	if e != nil {
		return nil, e
	}
	return i.space.c.rowCall("database.Update", r, b, ob)
}
func (i *Index) Upsert(v []any, ops []Operation) error {
	b, e := array(v)
	if e != nil {
		return e
	}
	ob, e := operations(ops, false)
	if e != nil {
		return e
	}
	r, e := i.resolve()
	if e != nil {
		return e
	}
	return i.space.c.do("database.Upsert", r, b, ob)
}

// ScanOptions defaults to equality for a non-empty key and all for an empty key.
// A nil Limit means unlimited; a pointer to zero requests zero rows.
type ScanOptions struct {
	Mode   *IteratorType
	Limit  *uint64
	Offset uint64
}

// IndexIterator owns a host iterator. Explicit users must defer Close.
type IndexIterator struct {
	resource
	progress *indexProgress
}

// Copies share scan progress as well as resource ownership.
type indexProgress struct {
	exhausted bool
	offset    uint64
	limited   bool
	remaining uint64
}

func (i *Index) Iter(k any, o ScanOptions) (*IndexIterator, error) {
	b, e := key(k)
	if e != nil {
		return nil, e
	}
	mode := IteratorTypeEqual
	if len(b) == 1 && b[0] == 0x90 {
		mode = IteratorTypeAll
	}
	if o.Mode != nil {
		mode = *o.Mode
	}
	if mode > IteratorTypePreviousPrefix {
		return nil, fmt.Errorf("twr: invalid iterator type %d", mode)
	}
	r, e := i.resolve()
	if e != nil {
		return nil, e
	}
	h, e := i.space.c.host.Call("index.IteratorNew", r, mode, b)
	if e != nil {
		return nil, e
	}
	progress := &indexProgress{offset: o.Offset, limited: o.Limit != nil}
	if o.Limit != nil {
		progress.remaining = *o.Limit
	}
	return &IndexIterator{resource: own(i.space.c, h, "index.iterator.ResourceDrop"), progress: progress}, nil
}
func (it *IndexIterator) Next() (out []any, err error) {
	if it.progress != nil && it.progress.exhausted {
		return nil, nil
	}
	h, e := it.handle()
	if e != nil {
		return nil, e
	}
	if it.progress.limited && it.progress.remaining == 0 {
		it.progress.exhausted = true
		return nil, it.Close()
	}
	for {
		v, e := it.client().host.Call("index.iterator.Next", h)
		if e != nil {
			return nil, errors.Join(e, it.Close())
		}
		if v == nil {
			it.progress.exhausted = true
			return nil, it.Close()
		}
		out, e = it.client().row(v)
		if e != nil {
			return nil, errors.Join(e, it.Close())
		}
		if it.progress.offset != 0 {
			it.progress.offset--
			continue
		}
		if it.progress.limited {
			it.progress.remaining--
			if it.progress.remaining == 0 {
				it.progress.exhausted = true
				return out, it.Close()
			}
		}
		return out, nil
	}
}

// Each closes its iterator even when fn returns false, an error, or panics.
func (i *Index) Each(k any, o ScanOptions, fn func([]any) (bool, error)) error {
	it, e := i.Iter(k, o)
	if e != nil {
		return e
	}
	return Using(it, func(it *IndexIterator) error {
		for {
			row, e := it.Next()
			if e != nil {
				return e
			}
			if row == nil {
				return nil
			}
			more, e := fn(row)
			if e != nil {
				return e
			}
			if !more {
				return nil
			}
		}
	})
}
func (i *Index) Select(k any, o ScanOptions) ([][]any, error) {
	out := make([][]any, 0)
	e := i.Each(k, o, func(row []any) (bool, error) { out = append(out, row); return true, nil })
	return out, e
}
