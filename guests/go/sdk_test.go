package twr

import (
	"errors"
	"fmt"
	"reflect"
	"testing"

	"github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/types"
)

type fake struct {
	fn func(string, ...any) (any, error)
}

func (f *fake) Call(op string, a ...any) (any, error) { return f.fn(op, a...) }
func TestSchemaAndRows(t *testing.T) {
	id := uint32(1)
	released := 0
	seen := []uint32{}
	f := &fake{fn: func(op string, a ...any) (any, error) {
		switch op {
		case "database.SpaceByName":
			return types.Space{ID: id}, nil
		case "database.IndexByName":
			s := a[0].(types.Space)
			return types.Index{ID: id + 10, SpaceID: s.ID}, nil
		case "index.Get":
			i := a[0].(types.Index)
			seen = append(seen, i.SpaceID)
			return types.BoxTuple{Handle: 1}, nil
		case "box-tuple.ToBytes":
			return []byte{0x92, 1, 0xa1, 'a'}, nil
		case "box-tuple.Release":
			released++
			return nil, nil
		}
		return nil, fmt.Errorf("unexpected %s", op)
	}}
	c := New(f)
	s := c.Space("users")
	i := s.Index("primary")
	for n := 0; n < 2; n++ {
		r, e := i.Get(uint64(1))
		if e != nil || !reflect.DeepEqual(r, []any{uint64(1), "a"}) {
			t.Fatal(r, e)
		}
		id++
	}
	if released != 2 || !reflect.DeepEqual(seen, []uint32{1, 2}) {
		t.Fatal(released, seen)
	}
}
func TestTupleDecodeCleanup(t *testing.T) {
	primary := errors.New("read")
	cleanup := errors.New("release")
	for _, mode := range []string{"read", "decode", "panic"} {
		released := 0
		c := New(&fake{fn: func(op string, a ...any) (any, error) {
			switch op {
			case "database.Insert":
				return types.BoxTuple{Handle: 1}, nil
			case "box-tuple.ToBytes":
				if mode == "panic" {
					panic(primary)
				}
				if mode == "read" {
					return nil, primary
				}
				return []byte{0xc1}, nil
			case "box-tuple.Release":
				released++
				return nil, cleanup
			}
			return nil, nil
		}})
		func() {
			defer func() {
				if p := recover(); p != nil && p != primary {
					t.Fatal(p)
				}
			}()
			_, e := c.Space(uint32(1)).Insert([]any{1})
			if mode == "read" && !errors.Is(e, primary) {
				t.Fatal(e)
			}
			if mode != "panic" && !errors.Is(e, cleanup) {
				t.Fatal(e)
			}
		}()
		if released != 1 {
			t.Fatal(mode, released)
		}
	}
}
func TestResourceCopies(t *testing.T) {
	released := 0
	c := New(&fake{fn: func(op string, a ...any) (any, error) {
		switch op {
		case "box-tuple.New":
			return types.BoxTuple{Handle: 1}, nil
		case "box-tuple.Release":
			released++
			return nil, nil
		case "box-tuple.Retain":
			return nil, nil
		case "box-tuple.FieldCount":
			return uint32(1), nil
		}
		return nil, nil
	}})
	r, e := c.NewTuple([]any{1})
	if e != nil {
		t.Fatal(e)
	}
	copy := *r
	owned, e := r.Copy()
	if e != nil {
		t.Fatal(e)
	}
	if e = r.Close(); e != nil {
		t.Fatal(e)
	}
	copy.Close()
	if _, e = copy.FieldCount(); !errors.Is(e, ErrClosed) {
		t.Fatal(e)
	}
	if _, e = owned.FieldCount(); e != nil {
		t.Fatal(e)
	}
	owned.Close()
	owned.Close()
	if released != 2 {
		t.Fatal(released)
	}
}
func TestIteratorEarlyExit(t *testing.T) {
	for _, mode := range []string{"return", "error", "panic", "exhaust"} {
		drops := 0
		next := 0
		primary := errors.New("callback")
		c := New(&fake{fn: func(op string, a ...any) (any, error) {
			switch op {
			case "index.IteratorNew":
				return uint32(1), nil
			case "index.iterator.Next":
				next++
				if next > 1 {
					return nil, nil
				}
				return types.BoxTuple{Handle: 1}, nil
			case "box-tuple.ToBytes":
				return []byte{0x91, 1}, nil
			case "box-tuple.Release":
				return nil, nil
			case "index.iterator.ResourceDrop":
				drops++
				return nil, nil
			}
			return nil, fmt.Errorf("unexpected %s", op)
		}})
		func() {
			defer func() {
				if p := recover(); p != nil && p != primary {
					t.Fatal(p)
				}
			}()
			e := c.Space(uint32(1)).Each(nil, ScanOptions{}, func([]any) (bool, error) {
				switch mode {
				case "panic":
					panic(primary)
				case "error":
					return false, primary
				case "return":
					return false, nil
				}
				return true, nil
			})
			if mode == "error" && !errors.Is(e, primary) {
				t.Fatal(e)
			}
		}()
		if drops != 1 {
			t.Fatal(mode, drops)
		}
	}
}
func TestUsingPreservesPrimary(t *testing.T) {
	primary := errors.New("primary")
	cleanup := errors.New("cleanup")
	c := New(&fake{fn: func(string, ...any) (any, error) { return nil, cleanup }})
	r := &Tuple{own(c, types.BoxTuple{}, "box-tuple.Release")}
	e := Using(r, func(*Tuple) error { return primary })
	if !errors.Is(e, primary) || !errors.Is(e, cleanup) {
		t.Fatal(e)
	}
	r = &Tuple{own(c, types.BoxTuple{}, "box-tuple.Release")}
	func() {
		defer func() {
			if recover() != primary {
				t.Fatal("panic replaced")
			}
		}()
		_ = Using(r, func(*Tuple) error { panic(primary) })
	}()
}
func TestTransactions(t *testing.T) {
	for _, mode := range []string{"commit", "error", "panic", "replaced", "manual", "nested"} {
		id := uint64(0)
		rollbacks := 0
		commits := 0
		primary := errors.New("primary")
		cleanup := errors.New("rollback")
		f := &fake{fn: func(op string, a ...any) (any, error) {
			switch op {
			case "transaction.ID":
				if id == 0 {
					return nil, nil
				}
				return id, nil
			case "transaction.Begin":
				id = 1
				return struct{}{}, nil
			case "transaction.Commit":
				commits++
				id = 0
				return struct{}{}, nil
			case "transaction.Rollback":
				rollbacks++
				id = 0
				return nil, cleanup
			}
			return struct{}{}, nil
		}}
		c := New(f)
		func() {
			defer func() {
				if p := recover(); p != nil && p != primary {
					t.Fatal(p)
				}
			}()
			e := c.Transaction(func() error {
				switch mode {
				case "error":
					return primary
				case "panic":
					panic(primary)
				case "replaced":
					id = 2
					return nil
				case "manual":
					if !errors.Is(c.Commit(), ErrManagedTransaction) || !errors.Is(c.Rollback(), ErrManagedTransaction) {
						t.Fatal("manual completion allowed")
					}
				case "nested":
					if !errors.Is(c.Transaction(func() error { return nil }, TransactionOptions{}), ErrNestedTransaction) {
						t.Fatal("nested allowed")
					}
				}
				return nil
			}, TransactionOptions{})
			switch mode {
			case "error":
				if !errors.Is(e, primary) || !errors.Is(e, cleanup) {
					t.Fatal(e)
				}
			case "replaced":
				if !errors.Is(e, ErrTransactionChanged) {
					t.Fatal(e)
				}
			default:
				if e != nil {
					t.Fatal(e)
				}
			}
		}()
		if c.managed {
			t.Fatal("helper state leaked")
		}
		switch mode {
		case "error", "panic":
			if rollbacks != 1 || commits != 0 {
				t.Fatal(rollbacks, commits)
			}
		case "replaced":
			if rollbacks != 0 || commits != 0 || id != 2 {
				t.Fatal(rollbacks, commits, id)
			}
		default:
			if commits != 1 || rollbacks != 0 {
				t.Fatal(commits, rollbacks)
			}
		}
	}
}
func TestErrors(t *testing.T) {
	file := "guest"
	line := uint32(42)
	e := &Error{Code: 999, Type: "ClientError", Message: "m", File: &file, Line: &line}
	c := New(&fake{fn: func(op string, a ...any) (any, error) {
		if op == "error.Last" {
			return errorRecord(e), nil
		}
		return nil, nil
	}})
	last, err := c.LastError()
	if err != nil || !reflect.DeepEqual(e, last) {
		t.Fatal(last, err)
	}
	if !errors.Is(last, &Error{Code: 999}) {
		t.Fatal(last)
	}
	var out *Error
	if !errors.As(fmt.Errorf("wrapped: %w", last), &out) {
		t.Fatal(last)
	}
}
func TestNoHostDuringConstruction(t *testing.T) {
	calls := 0
	c := New(&fake{fn: func(string, ...any) (any, error) { calls++; return nil, nil }})
	c.Space("users").Index("primary")
	c.Sequence(1)
	c.Session(1)
	if calls != 0 {
		t.Fatal(calls)
	}
}

func TestInvalidEnumsDoNotReachHost(t *testing.T) {
	c := New(&fake{fn: func(string, ...any) (any, error) {
		t.Fatal("invalid enum reached canonical ABI")
		return nil, nil
	}})
	invalid := IteratorType(255)
	_, count := c.Space(uint32(1)).Index(uint32(0)).Count(nil, invalid)
	_, iter := c.Space(uint32(1)).Iter(nil, ScanOptions{Mode: &invalid})
	_, key := c.NewKeyDef([]KeyPart{{FieldType: FieldType(255)}})
	for _, err := range []error{count, iter, key, c.SetIsolation(TransactionIsolation(255)), c.Log(LogLevel(255), "m", nil)} {
		if err == nil {
			t.Fatal("invalid enum accepted")
		}
	}
}

func TestNilPanicCleanup(t *testing.T) {
	closed, rolledBack := 0, 0
	active := false
	c := New(&fake{fn: func(op string, _ ...any) (any, error) {
		switch op {
		case "box-tuple.Release":
			closed++
		case "transaction.Begin":
			active = true
		case "transaction.ID":
			if active {
				return uint64(1), nil
			}
		case "transaction.Rollback":
			rolledBack++
			active = false
		default:
			t.Fatalf("unexpected %s", op)
		}
		return nil, nil
	}})
	for _, run := range []func(){
		func() {
			_ = Using(&Tuple{own(c, types.BoxTuple{}, "box-tuple.Release")}, func(*Tuple) error { panic(nil) })
		},
		func() { _ = c.Transaction(func() error { panic(nil) }, TransactionOptions{}) },
	} {
		returned := false
		func() {
			defer func() {
				_ = recover()
				if returned {
					t.Fatal("nil panic was swallowed")
				}
			}()
			run()
			returned = true
		}()
	}
	if closed != 1 || rolledBack != 1 || active || c.managed {
		t.Fatal(closed, rolledBack, active, c.managed)
	}
}

func TestIteratorPagination(t *testing.T) {
	for _, tc := range []struct {
		limit, offset uint64
		want          []uint64
		nextCalls     int
		releases      int
	}{
		{0, 0, nil, 0, 0},
		{1, 1, []uint64{2}, 2, 2},
		{2, 1, []uint64{2, 3}, 3, 3},
		{1, 5, nil, 4, 3},
	} {
		next, released, dropped := 0, 0, 0
		c := New(&fake{fn: func(op string, args ...any) (any, error) {
			switch op {
			case "index.IteratorNew":
				return uint32(1), nil
			case "index.iterator.Next":
				next++
				if next > 3 {
					return nil, nil
				}
				return types.BoxTuple{Handle: uint64(next)}, nil
			case "box-tuple.ToBytes":
				return []byte{0x91, byte(args[0].(types.BoxTuple).Handle)}, nil
			case "box-tuple.Release":
				released++
			case "index.iterator.ResourceDrop":
				dropped++
			default:
				t.Fatalf("unexpected %s", op)
			}
			return nil, nil
		}})
		options := ScanOptions{Limit: &tc.limit, Offset: tc.offset}
		it, err := c.Space(uint32(1)).Iter(nil, options)
		if err != nil {
			t.Fatal(err)
		}
		alias := *it
		var got []uint64
		for {
			row, err := alias.Next()
			if err != nil {
				t.Fatal(err)
			}
			if row == nil {
				break
			}
			got = append(got, row[0].(uint64))
			alias = *it // Progress must not reset when the Go value is copied.
		}
		if err := it.Close(); err != nil {
			t.Fatal(err)
		}
		if !reflect.DeepEqual(got, tc.want) || next != tc.nextCalls || released != tc.releases || dropped != 1 {
			t.Fatalf("pagination %+v: rows=%v next=%d releases=%d drops=%d", tc, got, next, released, dropped)
		}
		next, released, dropped = 0, 0, 0
		rows, err := c.Space(uint32(1)).Select(nil, options)
		if err != nil || len(rows) != len(tc.want) || next != tc.nextCalls || released != tc.releases || dropped != 1 {
			t.Fatalf("Select pagination %+v: %v %v next=%d releases=%d drops=%d", tc, rows, err, next, released, dropped)
		}
	}
}
