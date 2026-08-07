package main

import (
	"adder-wasm-bindings/internal/docs/adder/adder"
	tuple "adder-wasm-bindings/internal/tarantool/tarantool/box-tuple"
	"adder-wasm-bindings/internal/tarantool/tarantool/database"
	"adder-wasm-bindings/internal/tarantool/tarantool/types"
	"go.bytecodealliance.org/cm"
)

type Space = types.Space
type Index = types.Index
type BoxError = types.BoxError
type BoxTuple = types.BoxTuple
type MsgpackValue = types.MsgpackValue
type BoxErrorShape = adder.BoxErrorShape
type BytesResult = cm.Result[BoxErrorShape, MsgpackValue, BoxError]
type OptionalBytesResult = cm.Result[BoxErrorShape, cm.Option[MsgpackValue], BoxError]

func convertResult[FromShape, ToShape, Value, Err any](
	result cm.Result[FromShape, Value, Err],
) cm.Result[ToShape, Value, Err] {
	if result.IsErr() {
		return cm.Err[cm.Result[ToShape, Value, Err], ToShape, Value, Err](*result.Err())
	}
	return cm.OK[cm.Result[ToShape, Value, Err], ToShape, Value, Err](*result.OK())
}

func tupleToBytes(value BoxTuple) BytesResult {
	bytes := tuple.ToBytes(value)
	tuple.Release(value)
	return convertResult[tuple.BoxErrorShape, BoxErrorShape, MsgpackValue, BoxError](bytes)
}

func optionalTupleToBytes(
	result cm.Result[database.BoxErrorShape, cm.Option[BoxTuple], BoxError],
) OptionalBytesResult {
	if result.IsErr() {
		return cm.Err[OptionalBytesResult, BoxErrorShape, cm.Option[MsgpackValue], BoxError](*result.Err())
	}
	value := result.OK().Some()
	if value == nil {
		return cm.OK[OptionalBytesResult, BoxErrorShape, cm.Option[MsgpackValue], BoxError](cm.None[MsgpackValue]())
	}
	bytes := tupleToBytes(*value)
	if bytes.IsErr() {
		return cm.Err[OptionalBytesResult, BoxErrorShape, cm.Option[MsgpackValue], BoxError](*bytes.Err())
	}
	return cm.OK[OptionalBytesResult, BoxErrorShape, cm.Option[MsgpackValue], BoxError](cm.Some(*bytes.OK()))
}

func init() {
	adder.Exports.TSchemaVersion = database.SchemaVersion
	adder.Exports.TSpaceByName = func(name string) cm.Result[BoxErrorShape, cm.Option[Space], BoxError] {
		return convertResult[database.BoxErrorShape, BoxErrorShape, cm.Option[Space], BoxError](database.SpaceByName(name))
	}
	adder.Exports.TIndexByName = func(space Space, name string) cm.Result[BoxErrorShape, cm.Option[Index], BoxError] {
		return convertResult[database.BoxErrorShape, BoxErrorShape, cm.Option[Index], BoxError](database.IndexByName(space, name))
	}
	adder.Exports.TInsert = func(space Space, value MsgpackValue) BytesResult {
		result := database.Insert(space, value)
		if result.IsErr() {
			return cm.Err[BytesResult, BoxErrorShape, MsgpackValue, BoxError](*result.Err())
		}
		return tupleToBytes(*result.OK())
	}
	adder.Exports.TUpdate = func(index Index, key, operations MsgpackValue) OptionalBytesResult {
		return optionalTupleToBytes(database.Update(index, key, operations))
	}
	adder.Exports.TReplace = func(space Space, value MsgpackValue) BytesResult {
		result := database.Replace(space, value)
		if result.IsErr() {
			return cm.Err[BytesResult, BoxErrorShape, MsgpackValue, BoxError](*result.Err())
		}
		return tupleToBytes(*result.OK())
	}
	adder.Exports.TUpsert = database.Upsert
	adder.Exports.TDelete = func(index Index, key MsgpackValue) OptionalBytesResult {
		return optionalTupleToBytes(database.Delete(index, key))
	}
	adder.Exports.TTruncate = database.Truncate
}

func main() {}
