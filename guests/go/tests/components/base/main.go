package main

import (
	"bytes"
	"errors"
	world "example/go-base/internal/example/go-base/v0.1.0/sdk-test"
	"example/go-base/internal/wasi/cli/v0.2.12/run"
	"fmt"
	"math"

	twr "github.com/tarantool/tarantool-wasm-rs/guests/go"
	"github.com/tarantool/tarantool-wasm-rs/guests/go/codec"
	rawtx "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/transaction"
	"go.bytecodealliance.org/cm"
)

var users = twr.Box.Space("go_users")
var namedIndex = users.Index("name")

func must(e error) {
	if e != nil {
		panic(e)
	}
}
func assert(ok bool) {
	if !ok {
		panic("assertion failed")
	}
}
func rowsEqual(row []any, k uint64, s string) { assert(len(row) >= 2 && row[0] == k && row[1] == s) }
func check(name string) (result string) {
	defer func() {
		if p := recover(); p != nil {
			result = fmt.Sprint(p)
		}
	}()
	switch name {
	case "crud":
		must(users.Truncate())
		row, e := users.Insert([]any{uint64(1), "one"})
		must(e)
		rowsEqual(row, 1, "one")
		id, e := users.ID()
		must(e)
		row, e = twr.Box.Space(id).Get(uint64(1))
		must(e)
		rowsEqual(row, 1, "one")
		row, e = users.Replace([]any{uint64(1), "two"})
		must(e)
		rowsEqual(row, 1, "two")
		row, e = namedIndex.Get("two")
		must(e)
		rowsEqual(row, 1, "two")
		row, e = users.Update(uint64(1), []twr.Operation{{Operator: "=", Field: 1, Arguments: []any{"three"}}})
		must(e)
		rowsEqual(row, 1, "three")
		must(users.Upsert([]any{uint64(2), "four"}, []twr.Operation{}))
		row, e = users.Delete(uint64(1))
		must(e)
		rowsEqual(row, 1, "three")
		row, e = users.Get(uint64(999))
		must(e)
		assert(row == nil)
		_, e = users.Insert([]any{uint64(2), "duplicate"})
		var te *twr.Error
		assert(errors.As(e, &te))
		assert(te.Code != 0 && te.Type != "")
		assert(errors.Is(e, &twr.Error{Code: te.Code}))
	case "indexes":
		must(users.Truncate())
		for n := uint64(1); n <= 3; n++ {
			_, e := users.Insert([]any{n, fmt.Sprint(n)})
			must(e)
		}
		i := users.Index(0)
		_, e := i.ID()
		must(e)
		n, e := i.TupleCount()
		must(e)
		assert(n == 3)
		n, e = i.MemorySize()
		must(e)
		assert(n > 0)
		r, e := i.Min(nil)
		must(e)
		assert(r[0] == uint64(1))
		r, e = i.Max(nil)
		must(e)
		assert(r[0] == uint64(3))
		r, e = i.Random(42)
		must(e)
		assert(r != nil)
		n, e = i.Count(nil, twr.IteratorTypeAll)
		must(e)
		assert(n == 3)
		limit := uint64(1)
		rs, e := i.Select(nil, twr.ScanOptions{Offset: 1, Limit: &limit})
		must(e)
		assert(len(rs) == 1 && rs[0][0] == uint64(2))
		page, e := i.Iter(nil, twr.ScanOptions{Offset: 1, Limit: &limit})
		must(e)
		defer page.Close()
		r, e = page.Next()
		must(e)
		assert(r[0] == uint64(2))
		r, e = page.Next()
		must(e)
		assert(r == nil)
		must(page.Close())
		zero := uint64(0)
		empty, e := i.Iter(nil, twr.ScanOptions{Limit: &zero})
		must(e)
		defer empty.Close()
		r, e = empty.Next()
		must(e)
		assert(r == nil)
		must(empty.Close())
		seen := 0
		must(i.Each(nil, twr.ScanOptions{}, func(r []any) (bool, error) { seen++; return false, nil }))
		assert(seen == 1)
		it, e := i.Iter(nil, twr.ScanOptions{})
		must(e)
		defer it.Close()
		for {
			r, e := it.Next()
			must(e)
			if r == nil {
				break
			}
		}
		must(it.Close())
		must(it.Close())
	case "tuples":
		t, e := twr.Box.NewTuple([]any{uint64(1), "tuple", nil})
		must(e)
		defer t.Close()
		copy, e := t.Copy()
		must(e)
		must(t.Close())
		defer copy.Close()
		n, e := copy.FieldCount()
		must(e)
		assert(n == 3)
		size, e := copy.ByteSize()
		must(e)
		assert(size > 0)
		v, ok, e := copy.Field(2)
		must(e)
		assert(ok && v == nil)
		_, ok, e = copy.Field(99)
		must(e)
		assert(!ok)
		v, ok, e = copy.FieldByPath("[1]")
		must(e)
		assert(ok && v == "tuple")
		f, e := copy.Format()
		must(e)
		defer f.Close()
		must(copy.Validate(f))
		fc, e := f.Copy()
		must(e)
		must(fc.Close())
		it, e := copy.Cursor()
		must(e)
		defer it.Close()
		n, e = it.Position()
		must(e)
		assert(n == 0)
		_, ok, e = it.Seek(1)
		must(e)
		assert(ok)
		must(it.Rewind())
		for {
			_, ok, e := it.Read()
			must(e)
			if !ok {
				break
			}
		}
		must(it.Rewind())
		must(it.Close())
		must(copy.Each(func(v any) (bool, error) { return false, nil }))
		updated, e := copy.Update([]twr.Operation{{Operator: "=", Field: 1, Arguments: []any{"updated"}}})
		must(e)
		defer updated.Close()
		r, e := updated.Values()
		must(e)
		assert(r[1] == "updated")
		up, e := copy.Upsert([]twr.Operation{{Operator: "=", Field: 1, Arguments: []any{"upsert"}}})
		must(e)
		must(up.Close())
		_, e = t.FieldCount()
		assert(errors.Is(e, twr.ErrClosed))
	case "keys":
		k, e := twr.Box.NewKeyDef([]twr.KeyPart{{FieldNo: 0, FieldType: twr.FieldTypeUnsigned}})
		must(e)
		defer k.Close()
		n, e := k.PartCount()
		must(e)
		assert(n == 1)
		ps, e := k.Parts()
		must(e)
		assert(ps[0].FieldNo == 0)
		copy, e := k.Copy()
		must(e)
		defer copy.Close()
		merged, e := k.Merge(copy)
		must(e)
		defer merged.Close()
		must(k.ValidateKey(uint64(1), false))
		must(k.ValidateKey(uint64(1), true))
		t, e := twr.Box.NewTuple([]any{uint64(1), "a"})
		must(e)
		defer t.Close()
		t2, e := twr.Box.NewTuple([]any{uint64(2), "b"})
		must(e)
		defer t2.Close()
		must(k.ValidateTuple(t))
		key, e := k.ExtractKey(t)
		must(e)
		assert(key[0] == uint64(1))
		cmp, e := t.Compare(t2, k)
		must(e)
		assert(cmp < 0)
		cmp, e = t.CompareWithKey(uint64(1), k)
		must(e)
		assert(cmp == 0)
		f, e := twr.Box.NewTupleFormat(k)
		must(e)
		defer f.Close()
		must(t.Validate(f))
		f2, e := twr.Box.DefaultTupleFormat()
		must(e)
		must(f2.Close())
	case "sequences":
		row, e := twr.Box.Space("go_ids").Get(uint64(1))
		must(e)
		s := twr.Box.Sequence(uint32(row[1].(uint64)))
		must(s.Set(math.MaxInt64 - 1))
		n, e := s.Current()
		must(e)
		assert(n == math.MaxInt64-1)
		n, e = s.Next()
		must(e)
		assert(n == math.MaxInt64)
		must(s.Reset())
		_, e = s.Current()
		assert(e != nil)
	case "diagnostics":
		file := "guest.go"
		line := uint32(42)
		e, err := twr.Box.NewErrorWithLocation(999, "guest failure", &file, &line)
		must(err)
		assert(e.Code == 999 && e.File != nil && *e.File == file)
		must(twr.Box.SetError(e))
		last, err := twr.Box.LastError()
		must(err)
		assert(last.Code == 999)
		s, err := twr.Box.FormatError(last)
		must(err)
		assert(s != "")
		must(twr.Box.ClearError())
		last, err = twr.Box.LastError()
		must(err)
		assert(last == nil)
		_, err = twr.Box.NewError(1, "new")
		must(err)
	case "logging":
		must(twr.Box.Log(twr.LogLevelInfo, "Go SDK", &twr.LogContext{File: "guest.go", Line: 1}))
		n, e := twr.Box.Uptime()
		must(e)
		assert(n >= 0)
	case "messagepack":
		b, e := twr.Box.FromJSON(`[1,"a"]`)
		must(e)
		s, e := twr.Box.ToJSON(b)
		must(e)
		assert(s != "")
		v, e := twr.Unpack(b)
		must(e)
		assert(v.([]any)[0] == uint64(1))
		checkValues()
	case "extensions":
		v := []any{uint64(1), codec.Decimal{Digits: "12345678901234567890123456789012345678", Scale: 10}, codec.UUID{1, 2, 3}, codec.Datetime{Seconds: -1, Nanoseconds: 999999999, TZOffset: 180}, codec.Interval{Years: 2, Adjust: 2}, codec.ErrorValue{Stack: []codec.ErrorFrame{{Type: "ClientError", Message: "go", Code: 42}}}}
		row, e := twr.Box.Space("go_specials").Replace(v)
		must(e)
		assert(len(row) == 6)
		assert(row[1].(codec.Decimal).Digits == v[1].(codec.Decimal).Digits)
		assert(row[3].(codec.Datetime).Nanoseconds == 999999999)
		_, e = twr.Box.Space("go_bigints").Replace([]any{uint64(1), uint64(math.MaxUint64), int64(math.MinInt64), uint64(1 << 53)})
		must(e)
	case "sessions":
		s, e := twr.Box.CurrentSession()
		must(e)
		assert(s.ID > 0)
		r, e := twr.Box.Space("go_ids").Get(uint64(2))
		must(e)
		s = twr.Box.Session(r[1].(uint64))
		must(s.Send(map[uint64]any{0: uint64(0), 1: uint64(77)}, map[uint64]any{0x30: "sdk"}))
		must(s.Send(map[uint64]any{0: uint64(0), 1: uint64(78)}, nil))
		must(twr.Box.BroadcastLua("go_event", "{answer=42}"))
	case "transactionRejected":
		assert(twr.Box.Begin(twr.TransactionOptions{}) != nil)
	default:
		return "unknown check " + name
	}
	return "ok"
}
func checkValues() {
	for _, v := range []any{uint64(math.MaxUint64), int64(math.MinInt64), []byte{0, 255}, nil, []any{}, map[string]any{}, codec.Pairs{{Key: []any{uint64(1)}, Value: "array key"}}, codec.Extension{Type: 42, Data: []byte{1, 2, 3}}} {
		b, e := twr.Pack(v)
		must(e)
		out, e := twr.Unpack(b)
		must(e)
		b2, e := twr.Pack(out)
		must(e)
		assert(bytes.Equal(b, b2))
	}
}
func transactions() {
	isolation := twr.TransactionIsolationReadCommitted
	o := twr.TransactionOptions{Isolation: &isolation}
	must(twr.Box.Transaction(func() error {
		_, active, e := twr.Box.TransactionID()
		must(e)
		assert(active)
		level, active, e := twr.Box.TransactionIsolation()
		must(e)
		assert(active && level == isolation)
		assert(errors.Is(twr.Box.Commit(), twr.ErrManagedTransaction))
		assert(errors.Is(twr.Box.Rollback(), twr.ErrManagedTransaction))
		assert(errors.Is(twr.Box.Begin(o), twr.ErrNestedTransaction))
		assert(errors.Is(twr.Box.Transaction(func() error { return nil }, o), twr.ErrNestedTransaction))
		_, e = users.Replace([]any{uint64(11), "commit"})
		return e
	}, o))
	original := errors.New("rollback")
	e := twr.Box.Transaction(func() error { _, e := users.Replace([]any{uint64(12), "rollback"}); must(e); return original }, o)
	assert(errors.Is(e, original))
	func() {
		defer func() { assert(recover() == "original panic") }()
		_ = twr.Box.Transaction(func() error { _, e := users.Replace([]any{uint64(13), "panic"}); must(e); panic("original panic") }, o)
	}()
	must(twr.Box.Begin(o))
	_, e = users.Replace([]any{uint64(14), "explicit"})
	must(e)
	must(twr.Box.Commit())
	must(twr.Box.Begin(o))
	must(twr.Box.MakeSynchronous())
	must(twr.Box.Rollback())
	func() {
		returned := false
		defer func() { _ = recover(); assert(!returned) }()
		_ = twr.Box.Transaction(func() error {
			_, err := users.Replace([]any{uint64(15), "nil panic"})
			must(err)
			panic(nil)
		}, o)
		returned = true
	}()
	row, e := users.Get(uint64(15))
	must(e)
	assert(row == nil)
	e = twr.Box.Transaction(func() error {
		r := rawtx.Rollback()
		if r.Err() != nil {
			return fmt.Errorf("raw rollback failed")
		}
		r = rawtx.Begin()
		if r.Err() != nil {
			return fmt.Errorf("raw begin failed")
		}
		return nil
	}, o)
	assert(errors.Is(e, twr.ErrTransactionChanged))
	must(twr.Box.Rollback())
}
func init() {
	world.Exports.Check = check
	world.Exports.Roundtrip = func(b cm.List[uint8]) cm.List[uint8] {
		v, e := twr.Unpack(b.Slice())
		must(e)
		out, e := twr.Pack(v)
		must(e)
		return cm.ToList(out)
	}
	run.Exports.Run = func() cm.BoolResult { transactions(); return cm.BoolResult(0) }
}
func main() {}
