package main

import (
    "go.bytecodealliance.org/cm"
    "adder-wasm-bindings/internal/docs/adder/adder"
    kd "adder-wasm-bindings/internal/tarantool/tarantool/key-def"
    bt "adder-wasm-bindings/internal/tarantool/tarantool/box-tuple"
    "adder-wasm-bindings/internal/tarantool/tarantool/msgpack"
    "adder-wasm-bindings/internal/tarantool/tarantool/types"
    tupleFormat "adder-wasm-bindings/internal/tarantool/tarantool/tuple-format"
    "encoding/json"
)

type BoxErrorShape = adder.BoxErrorShape
type BoxError = types.BoxError
type BoxTuple = types.BoxTuple
type TupleField = types.TupleField
type TupleFormat = types.TupleFormat
type KeyDef = types.KeyDef
type KeyPart = types.KeyPart

func init() {
	adder.Exports.TNew = func(data cm.List[uint8]) (result cm.Result[BoxErrorShape, BoxTuple, BoxError]) {
        res := bt.New(data)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, BoxTuple, BoxError], BoxErrorShape, BoxTuple, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, BoxTuple, BoxError], BoxErrorShape, BoxTuple, BoxError](*res.Err())
    }

	adder.Exports.TRef =  func(t BoxTuple) (result int32) {
        return bt.Ref(t)
    }

	adder.Exports.TUnref =  func(t BoxTuple) {
        bt.Unref(t)
    }

	adder.Exports.TFieldCount =  func(t BoxTuple) (result uint32) {
        return bt.FieldCount(t)
    }

	adder.Exports.TBsize =  func(t BoxTuple) (result uint64) {
        return bt.Bsize(t)
    }

	adder.Exports.TToBuf =  func(t BoxTuple) (result cm.Result[BoxErrorShape, cm.List[uint8], BoxError]) {
        res := bt.ToBuf(t)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res.Err())
    }

	adder.Exports.TFormat =  func(t BoxTuple) (result TupleFormat) {
        return bt.Format(t)
    }

	adder.Exports.TField =  func(t BoxTuple, idx uint32) (result cm.Result[BoxErrorShape, TupleField, BoxError]) {
        res := bt.Field(t, idx)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, TupleField, BoxError], BoxErrorShape, TupleField, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, TupleField, BoxError], BoxErrorShape, TupleField, BoxError](*res.Err())
    }

	adder.Exports.TFieldByPath =  func(t BoxTuple, path string, indexBase int32) (result cm.Result[BoxErrorShape, TupleField, BoxError]) {
        res := bt.FieldByPath(t, path, indexBase)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, TupleField, BoxError], BoxErrorShape, TupleField, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, TupleField, BoxError], BoxErrorShape, TupleField, BoxError](*res.Err())
    }

	adder.Exports.TUpdate =  func(t BoxTuple, expr cm.List[uint8]) (result cm.Result[BoxErrorShape, BoxTuple, BoxError]) {
        res := bt.Update(t, expr)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, BoxTuple, BoxError], BoxErrorShape, BoxTuple, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, BoxTuple, BoxError], BoxErrorShape, BoxTuple, BoxError](*res.Err())
    }

	adder.Exports.TUpsert =  func(t BoxTuple, expr cm.List[uint8]) (result cm.Result[BoxErrorShape, BoxTuple, BoxError]) {
        res := bt.Upsert(t, expr)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, BoxTuple, BoxError], BoxErrorShape, BoxTuple, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, BoxTuple, BoxError], BoxErrorShape, BoxTuple, BoxError](*res.Err())
    }

	adder.Exports.TValidate =  func(t BoxTuple, format TupleFormat) (result bool) {
        return bt.Validate(t, format)
    }

	adder.Exports.TCompare =  func(keyDef KeyDef, left BoxTuple, right BoxTuple) (result int32) {
        return bt.Compare(keyDef, left, right)
    }

	adder.Exports.TCompareWithKey =  func(tuple BoxTuple, key cm.List[uint8], keyDef KeyDef) (result int32) {
        return bt.CompareWithKey(tuple, key, keyDef)
    }

	adder.Exports.TKeyDefNew =  func(parts cm.List[KeyPart]) (result cm.Result[BoxErrorShape, KeyDef, BoxError]) {
        res := kd.New(parts)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, KeyDef, BoxError], BoxErrorShape, KeyDef, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, KeyDef, BoxError], BoxErrorShape, KeyDef, BoxError](*res.Err())
    }

	adder.Exports.TTupleFormatDefault =  func() (result TupleFormat) {
        return tupleFormat.Default()
    }

	adder.Exports.TTupleFormatNew =  func(keyDefs cm.List[KeyDef]) (result cm.Result[BoxErrorShape, TupleFormat, BoxError]) {
        res := tupleFormat.New(keyDefs)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, TupleFormat, BoxError], BoxErrorShape, TupleFormat, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, TupleFormat, BoxError], BoxErrorShape, TupleFormat, BoxError](*res.Err())
    }

	adder.Exports.TDecodeFromRawPtr =  func(ptr uint64) (result cm.Result[BoxErrorShape, cm.List[uint8], BoxError]) {
        res := msgpack.DecodeFromRawPtr(ptr)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res.Err())
    }

	adder.Exports.TIterTestAll =  func(t BoxTuple) (result cm.Result[BoxErrorShape, cm.List[uint8], BoxError]) {
        iterRes := bt.TupleIteratorNewTupleIterator(t)
        if !iterRes.IsOK() {
            return cm.Err[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*iterRes.Err())
        }
        iter := *iterRes.OK()
    
        var res [][]any
    
        for {
            pos := iter.Position()
            nextRes := iter.Next()
            if !nextRes.IsOK() {
                break
            }
            field := *nextRes.OK()
            if field == 0 {
                break
            }
            res = append(res, []any{pos, field})
        }
    
        // rewind
        iter.Rewind()
        rpos := iter.Position()        
        rfield_result := iter.Next()
        var rfield TupleField
        if rfield_result.IsOK() {
			rfield = *rfield_result.OK()
		}
        res = append(res, []any{"rewind", rpos, rfield})
    
        // seek
        sfield_result := iter.Seek(1)
        var sfield TupleField
        if sfield_result.IsOK() {
			sfield = *sfield_result.OK()
		}

        spos := iter.Position()
        res = append(res, []any{"seek", spos, sfield})
    
        jsonBytes, err := json.Marshal(res)
        if err != nil {
            return cm.Err[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](BoxError{
                Code:    1,
                Message: err.Error(),
                Type:    "JsonError",
            })
        }
    
        msgpack_res := msgpack.Encode(cm.ToList(jsonBytes))
        if msgpack_res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*msgpack_res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*msgpack_res.Err())
    }
}

func main() {}
