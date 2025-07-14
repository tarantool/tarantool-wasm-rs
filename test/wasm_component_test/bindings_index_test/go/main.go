package main

import (
    "adder-wasm-bindings/internal/docs/adder/adder"
    "go.bytecodealliance.org/cm"
    idx "adder-wasm-bindings/internal/tarantool/tarantool/index"
    tuple "adder-wasm-bindings/internal/tarantool/tarantool/box-tuple"
    "adder-wasm-bindings/internal/tarantool/tarantool/types"
)

func main() {}

type Index = types.Index
type BoxError = types.BoxError
type BoxTuple = types.BoxTuple
type IteratorType = types.IteratorType
type BoxErrorShape = adder.BoxErrorShape

func init() {
	adder.Exports.TLen = func(index Index) cm.Result[BoxErrorShape, int64, BoxError] {
		res := idx.Len(index)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, int64, BoxError], BoxErrorShape, int64, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, int64, BoxError], BoxErrorShape, int64, BoxError](*res.Err())
	}

	adder.Exports.TBsize = func(index Index) cm.Result[BoxErrorShape, int64, BoxError] {
		res := idx.Bsize(index)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxErrorShape, int64, BoxError], BoxErrorShape, int64, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxErrorShape, int64, BoxError], BoxErrorShape, int64, BoxError](*res.Err())
	}

	adder.Exports.TRandom = func(index Index, rnd uint32) cm.Result[BoxErrorShape, cm.List[uint8], BoxError] {
		res := idx.Random(index, rnd)
		if res.IsOK() {
			res2 := tuple.ToBuf(*res.OK())
            if res2.IsOK() {
                return cm.OK[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res2.OK())
            }
            return cm.Err[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res2.Err()) 
		}
        return cm.Err[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res.Err()) 
	}

    adder.Exports.TGet = func(index Index, key cm.List[uint8]) cm.Result[BoxErrorShape, cm.List[uint8], BoxError] {
        res := idx.Get(index, key)
        if res.IsOK() {
            res2 := tuple.ToBuf(*res.OK())
            if res2.IsOK() {
                return cm.OK[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res2.OK())
            }
            return cm.Err[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res2.Err())
        }
        return cm.Err[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res.Err())
    }
    
    adder.Exports.TMin = func(index Index, key cm.List[uint8]) cm.Result[BoxErrorShape, cm.List[uint8], BoxError] {
        res := idx.Min(index, key)
        if res.IsOK() {
            res2 := tuple.ToBuf(*res.OK())
            if res2.IsOK() {
                return cm.OK[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res2.OK())
            }
            return cm.Err[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res2.Err())
        }
        return cm.Err[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res.Err())
    }
    
    adder.Exports.TMax = func(index Index, key cm.List[uint8]) cm.Result[BoxErrorShape, cm.List[uint8], BoxError] {
        res := idx.Max(index, key)
        if res.IsOK() {
            res2 := tuple.ToBuf(*res.OK())
            if res2.IsOK() {
                return cm.OK[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res2.OK())
            }
            return cm.Err[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res2.Err())
        }
        return cm.Err[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res.Err())
    }
    
    adder.Exports.TCount = func(index Index, iterType IteratorType, key cm.List[uint8]) cm.Result[BoxErrorShape, int64, BoxError] {
        res := idx.Count(index, iterType, key)
        if res.IsOK() {
            return cm.OK[cm.Result[BoxErrorShape, int64, BoxError], BoxErrorShape, int64, BoxError](*res.OK())
        }
        return cm.Err[cm.Result[BoxErrorShape, int64, BoxError], BoxErrorShape, int64, BoxError](*res.Err())
    }
    
}

