package main

import (
	"fmt"
	"encoding/json"
	"runtime"

	incominghandler "adder-wasm-bindings/internal/wasi/http/incoming-handler"
	"adder-wasm-bindings/internal/wasi/cli/run"
	wasiTypes "adder-wasm-bindings/internal/wasi/http/types"
	"adder-wasm-bindings/internal/tarantool/tarantool/say"
	"adder-wasm-bindings/internal/tarantool/tarantool/msgpack"
	"adder-wasm-bindings/internal/tarantool/tarantool/txn"
	tuple "adder-wasm-bindings/internal/tarantool/tarantool/box-tuple"
	box "adder-wasm-bindings/internal/tarantool/tarantool/ttbox"
	tarantoolTypes "adder-wasm-bindings/internal/tarantool/tarantool/types"
	"go.bytecodealliance.org/cm"
)

type LogContext = tarantoolTypes.LogContext
var LogContextNone = cm.None[LogContext]()

type Space = tarantoolTypes.Space
type Index = tarantoolTypes.Index
type BoxTuple = tarantoolTypes.BoxTuple

func init() {
	incominghandler.Exports.Handle = handleRequest
	run.Exports.Run = wasiRun
}

func handleRequest(request wasiTypes.IncomingRequest, responseOut wasiTypes.ResponseOutparam) {
	fmt.Println("Incoming HTTP request!")
}

func wasiRun() cm.BoolResult {
	main()
	return cm.BoolResult(true)
}

func Encode(obj interface{}) ([]byte, error) {
	jsonBytes, err := json.Marshal(obj)
	if err != nil {
		return nil, fmt.Errorf("error in JSON encoding: %v", err)
	}

	msgpackBytes := msgpack.Encode(cm.ToList(jsonBytes))
	if msgpackBytes.IsErr() {
		return nil, fmt.Errorf("error in msgpack encoding")
	}

	return msgpackBytes.OK().Slice(), nil
}

func Decode(data []byte) (interface{}, error) {
	decodedJsonBytes := msgpack.Decode(cm.ToList(data))
	if decodedJsonBytes.IsErr() {
		return nil, fmt.Errorf("error in msgpack decoding")
	}

	var obj interface{}
	err := json.Unmarshal(decodedJsonBytes.OK().Slice(), &obj)
	if err != nil {
		return nil, fmt.Errorf("error in JSON unmarshaling: %v", err)
	}

	return obj, nil
}

func _DecodeFromRawPtr(ptr BoxTuple) (interface{}, error) {
	decodedJsonBytes := msgpack.DecodeFromRawPtr(uint64(ptr))
	if decodedJsonBytes.IsErr() {
		return nil, fmt.Errorf("error in msgpack decoding")
	}

	var obj interface{}
	err := json.Unmarshal(decodedJsonBytes.OK().Slice(), &obj)
	if err != nil {
		return nil, fmt.Errorf("error in JSON unmarshaling: %v", err)
	}

	return obj, nil
}

// TODO: https://github.com/mandesero/tarantool-wasm-rs/issues/3
func GetTupleSlice(ptr BoxTuple) ([]byte, error) {
	result := tuple.ToBuf(ptr)
	if result.IsOK() {
		tup := *result.OK()
		return tup.Slice(), nil
	}
	return nil, fmt.Errorf("error")
}

func batchInsert(space Space, batch [][]byte) {
	var ptr BoxTuple
	txn.Begin()
	for _, elem := range batch {
		result := box.Insert(space, cm.ToList(elem))
		if result.IsOK() {
			ptr = *result.OK()
			raw, _ := GetTupleSlice(ptr)
			tup, _ := Decode(raw)
			say.SayInfo(fmt.Sprintf("%v", tup), cm.Some(LogContext {Filename: "main.go", Line: 104}))
		}
	}
	txn.Commit()
}

func getLogContext() cm.Option[LogContext] {
	_, file, line, _ := runtime.Caller(1)
	return cm.Some(LogContext {
		Filename: file,
		Line: 	  uint32(line),
	})
}

func main() {
	var space Space
	result1 := box.SpaceByName("test_space")
	if result1.IsOK() {
		space = *result1.OK()
	}

	say.SayInfo(fmt.Sprintf("%v", space), cm.Some(LogContext {Filename: "main.go", Line: 125}))

	tuples := make([][]byte, 5)

	for idx := range 5 {
		tuples[idx], _ = Encode([]int{idx})
		t, _ := Decode(tuples[idx])
		say.SayInfo(fmt.Sprintf("%v", t), cm.Some(LogContext {Filename: "main.go", Line: 132}))
	}

	batchInsert(space, tuples)

	var index Index
	result2 := box.IndexByName(space, "primary")
	if result2.IsOK() {
		index = *result2.OK()
	}
	
	say.SayInfo(fmt.Sprintf("%v", index), cm.Some(LogContext {Filename: "main.go", Line: 143}))
	
	var iter tarantoolTypes.Iterator
	empty_tuple, _ := Encode([]int{})
	result3 := tarantoolTypes.IteratorNewIterator(index, tarantoolTypes.IteratorTypeIterAll, cm.ToList(empty_tuple))
	if result2.IsOK() {
		iter = *result3.OK()
	} else {
		say.SayError(fmt.Sprintf("Iterator Error"), cm.Some(LogContext {Filename: "main.go", Line: 151}))
		return
	}

	for {
		r := iter.Next()
		if r.IsErr() {
			break
		}
		ptr := *r.OK()
		raw, _ := GetTupleSlice(ptr)
		tup, _ := Decode(raw)
		say.SayInfo(fmt.Sprintf("From iter %v", tup), cm.Some(LogContext {Filename: "main.go", Line: 163}))
	}
}
