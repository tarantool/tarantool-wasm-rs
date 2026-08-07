package main

import (
	"adder-wasm-bindings/internal/docs/adder/adder"
	tuple "adder-wasm-bindings/internal/tarantool/tarantool/box-tuple"
	idx "adder-wasm-bindings/internal/tarantool/tarantool/index"
	"adder-wasm-bindings/internal/tarantool/tarantool/types"
	"go.bytecodealliance.org/cm"
)

type Index = types.Index
type BoxError = types.BoxError
type BoxTuple = types.BoxTuple
type IteratorType = types.IteratorType
type MsgpackValue = types.MsgpackValue
type BoxErrorShape = adder.BoxErrorShape
type TupleResult = cm.Result[idx.BoxErrorShape, cm.Option[BoxTuple], BoxError]
type BytesResult = cm.Result[BoxErrorShape, cm.Option[MsgpackValue], BoxError]
type Uint64Result = cm.Result[BoxErrorShape, uint64, BoxError]

func tupleResultToBytes(result TupleResult) BytesResult {
	if result.IsErr() {
		return cm.Err[BytesResult, BoxErrorShape, cm.Option[MsgpackValue], BoxError](*result.Err())
	}

	value := result.OK().Some()
	if value == nil {
		return cm.OK[BytesResult, BoxErrorShape, cm.Option[MsgpackValue], BoxError](cm.None[MsgpackValue]())
	}

	bytes := tuple.ToBytes(*value)
	tuple.Release(*value)
	if bytes.IsErr() {
		return cm.Err[BytesResult, BoxErrorShape, cm.Option[MsgpackValue], BoxError](*bytes.Err())
	}
	return cm.OK[BytesResult, BoxErrorShape, cm.Option[MsgpackValue], BoxError](cm.Some(*bytes.OK()))
}

func uint64Result(result cm.Result[idx.BoxErrorShape, uint64, BoxError]) Uint64Result {
	if result.IsErr() {
		return cm.Err[Uint64Result, BoxErrorShape, uint64, BoxError](*result.Err())
	}
	return cm.OK[Uint64Result, BoxErrorShape, uint64, BoxError](*result.OK())
}

func init() {
	adder.Exports.TLen = func(index Index) cm.Result[BoxErrorShape, uint64, BoxError] {
		return uint64Result(idx.TupleCount(index))
	}

	adder.Exports.TBsize = func(index Index) cm.Result[BoxErrorShape, uint64, BoxError] {
		return uint64Result(idx.MemorySize(index))
	}

	adder.Exports.TRandom = func(index Index, rnd uint32) BytesResult {
		return tupleResultToBytes(idx.Random(index, rnd))
	}

	adder.Exports.TGet = func(index Index, key MsgpackValue) BytesResult {
		return tupleResultToBytes(idx.Get(index, key))
	}

	adder.Exports.TMin = func(index Index, key MsgpackValue) BytesResult {
		return tupleResultToBytes(idx.Min(index, key))
	}

	adder.Exports.TMax = func(index Index, key MsgpackValue) BytesResult {
		return tupleResultToBytes(idx.Max(index, key))
	}

	adder.Exports.TCount = func(index Index, iterType IteratorType, key MsgpackValue) cm.Result[BoxErrorShape, uint64, BoxError] {
		return uint64Result(idx.Count(index, iterType, key))
	}
}

func main() {}
