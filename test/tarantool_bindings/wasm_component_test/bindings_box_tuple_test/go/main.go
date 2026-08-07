package main

import (
	"encoding/json"

	"adder-wasm-bindings/internal/docs/adder/adder"
	bt "adder-wasm-bindings/internal/tarantool/tarantool/box-tuple"
	kd "adder-wasm-bindings/internal/tarantool/tarantool/key-def"
	"adder-wasm-bindings/internal/tarantool/tarantool/msgpack"
	tupleFormat "adder-wasm-bindings/internal/tarantool/tarantool/tuple-format"
	"adder-wasm-bindings/internal/tarantool/tarantool/types"
	"go.bytecodealliance.org/cm"
)

type BoxErrorShape = adder.BoxErrorShape
type BoxError = types.BoxError
type BoxTuple = types.BoxTuple
type TupleFormat = types.TupleFormat
type KeyDef = types.KeyDef
type KeyPart = types.KeyPart
type MsgpackValue = types.MsgpackValue
type MsgpackResult = cm.Result[BoxErrorShape, MsgpackValue, BoxError]

func convertResult[FromShape, ToShape, Value, Err any](
	result cm.Result[FromShape, Value, Err],
) cm.Result[ToShape, Value, Err] {
	if result.IsErr() {
		return cm.Err[cm.Result[ToShape, Value, Err], ToShape, Value, Err](*result.Err())
	}
	return cm.OK[cm.Result[ToShape, Value, Err], ToShape, Value, Err](*result.OK())
}

func iteratorResult(t BoxTuple) MsgpackResult {
	created := bt.IteratorNew(t)
	if created.IsErr() {
		return cm.Err[MsgpackResult, BoxErrorShape, MsgpackValue, BoxError](*created.Err())
	}
	iterator := *created.OK()
	defer iterator.ResourceDrop()

	result := make([][]any, 0, bt.FieldCount(t)+2)
	appendField := func(prefix []any, field MsgpackValue) *BoxError {
		decoded := msgpack.ToJSON(field)
		if decoded.IsErr() {
			return decoded.Err()
		}
		result = append(result, append(prefix, json.RawMessage(*decoded.OK())))
		return nil
	}

	for {
		position := iterator.Position()
		next := iterator.Next()
		if next.IsErr() {
			return cm.Err[MsgpackResult, BoxErrorShape, MsgpackValue, BoxError](*next.Err())
		}
		field := next.OK().Some()
		if field == nil {
			break
		}
		if err := appendField([]any{position}, *field); err != nil {
			return cm.Err[MsgpackResult, BoxErrorShape, MsgpackValue, BoxError](*err)
		}
	}

	iterator.Rewind()
	rewindPosition := iterator.Position()
	rewind := iterator.Next()
	if rewind.IsErr() {
		return cm.Err[MsgpackResult, BoxErrorShape, MsgpackValue, BoxError](*rewind.Err())
	}
	if field := rewind.OK().Some(); field != nil {
		if err := appendField([]any{"rewind", rewindPosition}, *field); err != nil {
			return cm.Err[MsgpackResult, BoxErrorShape, MsgpackValue, BoxError](*err)
		}
	}

	seek := iterator.Seek(1)
	if seek.IsErr() {
		return cm.Err[MsgpackResult, BoxErrorShape, MsgpackValue, BoxError](*seek.Err())
	}
	if field := seek.OK().Some(); field != nil {
		if err := appendField([]any{"seek", iterator.Position()}, *field); err != nil {
			return cm.Err[MsgpackResult, BoxErrorShape, MsgpackValue, BoxError](*err)
		}
	}

	encoded, err := json.Marshal(result)
	if err != nil {
		panic(err)
	}
	return convertResult[msgpack.BoxErrorShape, BoxErrorShape, MsgpackValue, BoxError](
		msgpack.FromJSON(string(encoded)),
	)
}

func init() {
	adder.Exports.TNew = func(data MsgpackValue) cm.Result[BoxErrorShape, BoxTuple, BoxError] {
		return convertResult[bt.BoxErrorShape, BoxErrorShape, BoxTuple, BoxError](bt.New(data))
	}
	adder.Exports.TRetain = bt.Retain
	adder.Exports.TRelease = bt.Release
	adder.Exports.TFieldCount = bt.FieldCount
	adder.Exports.TByteSize = bt.ByteSize
	adder.Exports.TToBytes = func(t BoxTuple) MsgpackResult {
		return convertResult[bt.BoxErrorShape, BoxErrorShape, MsgpackValue, BoxError](bt.ToBytes(t))
	}
	adder.Exports.TFormat = bt.Format
	adder.Exports.TField = func(t BoxTuple, index uint32) cm.Result[BoxErrorShape, cm.Option[MsgpackValue], BoxError] {
		return convertResult[bt.BoxErrorShape, BoxErrorShape, cm.Option[MsgpackValue], BoxError](bt.Field(t, index))
	}
	adder.Exports.TFieldByPath = func(t BoxTuple, path string) cm.Result[BoxErrorShape, cm.Option[MsgpackValue], BoxError] {
		return convertResult[bt.BoxErrorShape, BoxErrorShape, cm.Option[MsgpackValue], BoxError](bt.FieldByPath(t, path))
	}
	adder.Exports.TUpdate = func(t BoxTuple, expression MsgpackValue) cm.Result[BoxErrorShape, BoxTuple, BoxError] {
		return convertResult[bt.BoxErrorShape, BoxErrorShape, BoxTuple, BoxError](bt.Update(t, expression))
	}
	adder.Exports.TUpsert = func(t BoxTuple, expression MsgpackValue) cm.Result[BoxErrorShape, BoxTuple, BoxError] {
		return convertResult[bt.BoxErrorShape, BoxErrorShape, BoxTuple, BoxError](bt.Upsert(t, expression))
	}
	adder.Exports.TValidate = bt.Validate
	adder.Exports.TCompare = bt.Compare
	adder.Exports.TCompareWithKey = func(tuple BoxTuple, key MsgpackValue, keyDef KeyDef) cm.Result[BoxErrorShape, int32, BoxError] {
		return convertResult[bt.BoxErrorShape, BoxErrorShape, int32, BoxError](bt.CompareWithKey(tuple, key, keyDef))
	}
	adder.Exports.TKeyDefNew = func(parts cm.List[KeyPart]) cm.Result[BoxErrorShape, KeyDef, BoxError] {
		return convertResult[kd.BoxErrorShape, BoxErrorShape, KeyDef, BoxError](kd.New(parts))
	}
	adder.Exports.TTupleFormatDefault = tupleFormat.Default
	adder.Exports.TTupleFormatNew = func(keyDefs cm.List[KeyDef]) cm.Result[BoxErrorShape, TupleFormat, BoxError] {
		return convertResult[tupleFormat.BoxErrorShape, BoxErrorShape, TupleFormat, BoxError](tupleFormat.New(keyDefs))
	}
	adder.Exports.TIterTestAll = iteratorResult
}

func main() {}
