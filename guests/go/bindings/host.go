package bindings

import (
	"fmt"

	boxtuple "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/box-tuple"
	database "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/database"
	rawerror "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/error"
	index "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/index"
	keydef "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/key-def"
	log "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/log"
	metrics "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/metrics"
	msgpack "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/msgpack"
	sequence "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/sequence"
	session "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/session"
	transaction "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/transaction"
	tupleformat "github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/tuple-format"
	"github.com/tarantool/tarantool-wasm-rs/guests/go/raw/tarantool/tarantool/types"
	"go.bytecodealliance.org/cm"
)

// Host adapts canonical ABI values without accessing the host during initialization.
type Host struct{}

func option[T any](v any) cm.Option[T] {
	if v == nil {
		return cm.None[T]()
	}
	p := v.(*T)
	if p == nil {
		return cm.None[T]()
	}
	return cm.Some(*p)
}
func optional[T any](v cm.Option[T]) any {
	if p := v.Some(); p != nil {
		return *p
	}
	return nil
}
func (Host) Call(operation string, a ...any) (any, error) {
	switch operation {
	case "metrics.Uptime":
		r := metrics.Uptime()
		return r, nil
	case "transaction.ID":
		r := transaction.ID()
		return optional(r), nil
	case "transaction.Isolation":
		r := transaction.Isolation()
		return optional(r), nil
	case "transaction.Begin":
		r := transaction.Begin()
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "transaction.Commit":
		r := transaction.Commit()
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "transaction.Rollback":
		r := transaction.Rollback()
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "transaction.SetIsolation":
		r := transaction.SetIsolation(a[0].(transaction.TransactionIsolation))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "transaction.MakeSynchronous":
		transaction.MakeSynchronous()
		return nil, nil
	case "database.SchemaVersion":
		r := database.SchemaVersion()
		return r, nil
	case "database.SpaceByName":
		r := database.SpaceByName(a[0].(string))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return optional(*r.OK()), nil
	case "database.IndexByName":
		r := database.IndexByName(a[0].(database.Space), a[1].(string))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return optional(*r.OK()), nil
	case "database.Insert":
		r := database.Insert(a[0].(database.Space), types.MsgpackValue(cm.ToList(a[1].([]byte))))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "database.Update":
		r := database.Update(a[0].(database.Index), types.MsgpackValue(cm.ToList(a[1].([]byte))), types.MsgpackValue(cm.ToList(a[2].([]byte))))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return optional(*r.OK()), nil
	case "database.Replace":
		r := database.Replace(a[0].(database.Space), types.MsgpackValue(cm.ToList(a[1].([]byte))))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "database.Upsert":
		r := database.Upsert(a[0].(database.Index), types.MsgpackValue(cm.ToList(a[1].([]byte))), types.MsgpackValue(cm.ToList(a[2].([]byte))))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "database.Delete":
		r := database.Delete(a[0].(database.Index), types.MsgpackValue(cm.ToList(a[1].([]byte))))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return optional(*r.OK()), nil
	case "database.Truncate":
		r := database.Truncate(a[0].(database.Space))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "msgpack.FromJSON":
		r := msgpack.FromJSON(a[0].(string))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return r.OK().Slice(), nil
	case "msgpack.ToJSON":
		r := msgpack.ToJSON(types.MsgpackValue(cm.ToList(a[0].([]byte))))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "tuple-format.Default":
		r := tupleformat.Default()
		return r, nil
	case "tuple-format.New":
		r := tupleformat.New(cm.ToList(a[0].([]tupleformat.KeyDef)))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "tuple-format.Retain":
		tupleformat.Retain(a[0].(tupleformat.TupleFormat))
		return nil, nil
	case "tuple-format.Release":
		tupleformat.Release(a[0].(tupleformat.TupleFormat))
		return nil, nil
	case "key-def.New":
		r := keydef.New(cm.ToList(a[0].([]keydef.KeyPart)))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "key-def.Duplicate":
		r := keydef.Duplicate(a[0].(keydef.KeyDef))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "key-def.Merge":
		r := keydef.Merge(a[0].(keydef.KeyDef), a[1].(keydef.KeyDef))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "key-def.Release":
		keydef.Release(a[0].(keydef.KeyDef))
		return nil, nil
	case "key-def.DumpParts":
		r := keydef.DumpParts(a[0].(keydef.KeyDef))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return r.OK().Slice(), nil
	case "key-def.ValidateKey":
		r := keydef.ValidateKey(a[0].(keydef.KeyDef), types.MsgpackValue(cm.ToList(a[1].([]byte))))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "key-def.ValidateFullKey":
		r := keydef.ValidateFullKey(a[0].(keydef.KeyDef), types.MsgpackValue(cm.ToList(a[1].([]byte))))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "key-def.PartCount":
		r := keydef.PartCount(a[0].(keydef.KeyDef))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "key-def.ExtractKey":
		r := keydef.ExtractKey(a[0].(keydef.KeyDef), a[1].(keydef.BoxTuple))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return r.OK().Slice(), nil
	case "key-def.ValidateTuple":
		r := keydef.ValidateTuple(a[0].(keydef.KeyDef), a[1].(keydef.BoxTuple))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "index.iterator.ResourceDrop":
		a[0].(index.Iterator).ResourceDrop()
		return nil, nil
	case "index.IteratorNew":
		r := index.IteratorNew(a[0].(index.Index), a[1].(index.IteratorType), types.MsgpackValue(cm.ToList(a[2].([]byte))))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "index.iterator.Next":
		r := a[0].(index.Iterator).Next()
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return optional(*r.OK()), nil
	case "index.TupleCount":
		r := index.TupleCount(a[0].(index.Index))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "index.MemorySize":
		r := index.MemorySize(a[0].(index.Index))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "index.Random":
		r := index.Random(a[0].(index.Index), a[1].(uint32))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return optional(*r.OK()), nil
	case "index.Get":
		r := index.Get(a[0].(index.Index), types.MsgpackValue(cm.ToList(a[1].([]byte))))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return optional(*r.OK()), nil
	case "index.Min":
		r := index.Min(a[0].(index.Index), types.MsgpackValue(cm.ToList(a[1].([]byte))))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return optional(*r.OK()), nil
	case "index.Max":
		r := index.Max(a[0].(index.Index), types.MsgpackValue(cm.ToList(a[1].([]byte))))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return optional(*r.OK()), nil
	case "index.Count":
		r := index.Count(a[0].(index.Index), a[1].(index.IteratorType), types.MsgpackValue(cm.ToList(a[2].([]byte))))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "log.Write":
		log.Write(a[0].(log.LogLevel), a[1].(string), option[log.LogContext](a[2]))
		return nil, nil
	case "sequence.Next":
		r := sequence.Next(a[0].(sequence.Sequence))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "sequence.Current":
		r := sequence.Current(a[0].(sequence.Sequence))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "sequence.Set":
		r := sequence.Set(a[0].(sequence.Sequence), a[1].(int64))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "sequence.Reset":
		r := sequence.Reset(a[0].(sequence.Sequence))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "error.New":
		r := rawerror.New(rawerror.ErrorCode(a[0].(uint32)), a[1].(string))
		return r, nil
	case "error.NewWithLocation":
		r := rawerror.NewWithLocation(rawerror.ErrorCode(a[0].(uint32)), a[1].(string), option[string](a[2]), option[uint32](a[3]))
		return r, nil
	case "error.Set":
		rawerror.Set(a[0].(rawerror.BoxError))
		return nil, nil
	case "error.Last":
		r := rawerror.Last()
		return optional(r), nil
	case "error.Clear":
		rawerror.Clear()
		return nil, nil
	case "error.ToString":
		r := rawerror.ToString(a[0].(rawerror.BoxError))
		return r, nil
	case "box-tuple.iterator.ResourceDrop":
		a[0].(boxtuple.Iterator).ResourceDrop()
		return nil, nil
	case "box-tuple.IteratorNew":
		r := boxtuple.IteratorNew(a[0].(boxtuple.BoxTuple))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "box-tuple.iterator.Next":
		r := a[0].(boxtuple.Iterator).Next()
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return optional(*r.OK()), nil
	case "box-tuple.iterator.Position":
		r := a[0].(boxtuple.Iterator).Position()
		return r, nil
	case "box-tuple.iterator.Rewind":
		a[0].(boxtuple.Iterator).Rewind()
		return nil, nil
	case "box-tuple.iterator.Seek":
		r := a[0].(boxtuple.Iterator).Seek(a[1].(uint32))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return optional(*r.OK()), nil
	case "box-tuple.New":
		r := boxtuple.New(types.MsgpackValue(cm.ToList(a[0].([]byte))))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "box-tuple.Retain":
		boxtuple.Retain(a[0].(boxtuple.BoxTuple))
		return nil, nil
	case "box-tuple.Release":
		boxtuple.Release(a[0].(boxtuple.BoxTuple))
		return nil, nil
	case "box-tuple.FieldCount":
		r := boxtuple.FieldCount(a[0].(boxtuple.BoxTuple))
		return r, nil
	case "box-tuple.ByteSize":
		r := boxtuple.ByteSize(a[0].(boxtuple.BoxTuple))
		return r, nil
	case "box-tuple.ToBytes":
		r := boxtuple.ToBytes(a[0].(boxtuple.BoxTuple))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return r.OK().Slice(), nil
	case "box-tuple.Format":
		r := boxtuple.Format(a[0].(boxtuple.BoxTuple))
		return r, nil
	case "box-tuple.Field":
		r := boxtuple.Field(a[0].(boxtuple.BoxTuple), a[1].(uint32))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return optional(*r.OK()), nil
	case "box-tuple.FieldByPath":
		r := boxtuple.FieldByPath(a[0].(boxtuple.BoxTuple), a[1].(string))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return optional(*r.OK()), nil
	case "box-tuple.Update":
		r := boxtuple.Update(a[0].(boxtuple.BoxTuple), types.MsgpackValue(cm.ToList(a[1].([]byte))))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "box-tuple.Upsert":
		r := boxtuple.Upsert(a[0].(boxtuple.BoxTuple), types.MsgpackValue(cm.ToList(a[1].([]byte))))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "box-tuple.Validate":
		r := boxtuple.Validate(a[0].(boxtuple.BoxTuple), a[1].(boxtuple.TupleFormat))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "box-tuple.Compare":
		r := boxtuple.Compare(a[0].(boxtuple.KeyDef), a[1].(boxtuple.BoxTuple), a[2].(boxtuple.BoxTuple))
		return r, nil
	case "box-tuple.CompareWithKey":
		r := boxtuple.CompareWithKey(a[0].(boxtuple.BoxTuple), types.MsgpackValue(cm.ToList(a[1].([]byte))), a[2].(boxtuple.KeyDef))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "session.Current":
		r := session.Current()
		return r, nil
	case "session.Broadcast":
		r := session.Broadcast(a[0].(string), a[1].(string))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	case "session.IprotoSend":
		r := session.IprotoSend(a[0].(session.Session), types.MsgpackValue(cm.ToList(a[1].([]byte))), option[session.MsgpackValue](a[2]))
		if e := r.Err(); e != nil {
			return nil, FromRecord(*e)
		}
		return *r.OK(), nil
	}
	return nil, fmt.Errorf("unknown host operation %s", operation)
}
