package main

import (
    "adder-wasm-bindings/internal/docs/adder/adder"
    "adder-wasm-bindings/internal/tarantool/tarantool/ttbox"
    tuple "adder-wasm-bindings/internal/tarantool/tarantool/box-tuple"
    "adder-wasm-bindings/internal/tarantool/tarantool/types"
    "go.bytecodealliance.org/cm"
)

func main() {}

type Space = types.Space
type Index = types.Index
type BoxError = types.BoxError
type BoxErrorShape = adder.BoxErrorShape

func init() {
    adder.Exports.TSchemaVersion = func() (result int64) {
        return ttbox.SchemaVersion()
    }

    adder.Exports.TSpaceByName = func(name string) (result cm.Result[BoxErrorShape, Space, BoxError]) {
        res := ttbox.SpaceByName(name)
        if res.IsOK() {
            return cm.OK[cm.Result[BoxErrorShape, Space, BoxError], BoxErrorShape, Space, BoxError](*res.OK())
        }
        return cm.Err[cm.Result[BoxErrorShape, Space, BoxError], BoxErrorShape, Space, BoxError](*res.Err())
    }

    adder.Exports.TIndexByName = func(space Space, name string) (result cm.Result[BoxErrorShape, Index, BoxError]) {
        res := ttbox.IndexByName(space, name)
        if res.IsOK() {
            return cm.OK[cm.Result[BoxErrorShape, Index, BoxError], BoxErrorShape, Index, BoxError](*res.OK())
        }
        return cm.Err[cm.Result[BoxErrorShape, Index, BoxError], BoxErrorShape, Index, BoxError](*res.Err())
    }

    adder.Exports.TInsert = func(space Space, tup cm.List[uint8]) (result cm.Result[BoxErrorShape, cm.List[uint8], BoxError]) {
        res := ttbox.Insert(space, tup)
        if res.IsOK() {
            return convertTupleToBuf(*res.OK())
        }
        return cm.Err[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res.Err())
    }

    adder.Exports.TUpdate = func(index Index, key, ops cm.List[uint8]) (result cm.Result[BoxErrorShape, cm.List[uint8], BoxError]) {
        res := ttbox.Update(index, key, ops)
        if res.IsOK() {
            return convertTupleToBuf(*res.OK())
        }
        return cm.Err[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res.Err())
    }

    adder.Exports.TReplace = func(space Space, tup cm.List[uint8]) (result cm.Result[BoxErrorShape, cm.List[uint8], BoxError]) {
        res := ttbox.Replace(space, tup)
        if res.IsOK() {
            return convertTupleToBuf(*res.OK())
        }
        return cm.Err[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res.Err())
    }

    adder.Exports.TUpsert = func(index Index, tup, ops cm.List[uint8]) (result cm.Result[BoxError, struct{}, BoxError]) {
        res := ttbox.Upsert(index, tup, ops)
        if res.IsOK() {
            return cm.OK[cm.Result[BoxError, struct{}, BoxError], BoxError, struct{}, BoxError](*res.OK())
        }
        return cm.Err[cm.Result[BoxError, struct{}, BoxError], BoxError, struct{}, BoxError](*res.Err())
    }

    adder.Exports.TDelete = func(index Index, key cm.List[uint8]) (result cm.Result[BoxErrorShape, cm.List[uint8], BoxError]) {
        res := ttbox.Delete(index, key)
        if res.IsOK() {
            return convertTupleToBuf(*res.OK())
        }
        return cm.Err[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res.Err())
    }

    adder.Exports.TTruncate = func(space Space) (result cm.Result[BoxError, struct{}, BoxError]) {
        res := ttbox.Truncate(space)
        if res.IsOK() {
            return cm.OK[cm.Result[BoxError, struct{}, BoxError], BoxError, struct{}, BoxError](*res.OK())
        }
        return cm.Err[cm.Result[BoxError, struct{}, BoxError], BoxError, struct{}, BoxError](*res.Err())
    }
}

func convertTupleToBuf(t types.BoxTuple) (result cm.Result[BoxErrorShape, cm.List[uint8], BoxError]) {
    res := tuple.ToBuf(t)
    if res.IsOK() {
        return cm.OK[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res.OK())
    }
    return cm.Err[cm.Result[BoxErrorShape, cm.List[uint8], BoxError], BoxErrorShape, cm.List[uint8], BoxError](*res.Err())
}
