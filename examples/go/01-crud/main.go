package main

import (
	"fmt"
	"encoding/json"
	"runtime"

	incominghandler "example.com/internal/wasi/http/incoming-handler"
	wasiTypes "example.com/internal/wasi/http/types"
	"example.com/internal/tarantool/tarantool/say"
	"example.com/internal/tarantool/tarantool/msgpack"
	"example.com/internal/tarantool/tarantool/txn"
	box "example.com/internal/tarantool/tarantool/ttbox"
	tarantoolTypes "example.com/internal/tarantool/tarantool/types"
	// "example.com/internal/tarantool/tarantool/tuple"
	"go.bytecodealliance.org/cm"
)

type LogContext = tarantoolTypes.LogContext
var LogContextNone = cm.None[LogContext]()

type Space = tarantoolTypes.Space
type Index = tarantoolTypes.Index
type BoxTuple = tarantoolTypes.BoxTuple

func init() {
	incominghandler.Exports.Handle = handleRequest
}

func handleRequest(request wasiTypes.IncomingRequest, responseOut wasiTypes.ResponseOutparam) {
	fmt.Println("Incoming HTTP request!")
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
// func GetTupleSlice(ptr BoxTuple) ([]byte, error) {
// 	result := tuple.ToBuf(ptr)
// 	if result.IsOK() {
// 		tup := *result.OK()
// 		return tup.Slice(), nil
// 	}
// 	return nil, fmt.Errorf("error")
// }

func batchInsert(space Space, batch [][]byte) {
	var ptr BoxTuple
	txn.Begin()
	for _, elem := range batch {
		result := box.Insert(space, cm.ToList(elem))
		if result.IsOK() {
			ptr = *result.OK()
			// raw, _ := GetTupleSlice(ptr)
			// tup, _ := Decode(raw)
			say.SayInfo(fmt.Sprintf("%v", ptr), cm.Some(LogContext {Filename: "main.go", Line: 85}))
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

	say.SayInfo(fmt.Sprintf("%v", space), cm.Some(LogContext {Filename: "main.go", Line: 106}))

	tuples := make([][]byte, 5)

	for idx := range 5 {
		tuples[idx], _ = Encode([]int{idx})
		t, _ := Decode(tuples[idx])
		say.SayInfo(fmt.Sprintf("%v", t), cm.Some(LogContext {Filename: "main.go", Line: 133}))
	}

	batchInsert(space, tuples)

	var index Index
	result2 := box.IndexByName(space, "primary")
	if result2.IsOK() {
		index = *result2.OK()
	}
	
	say.SayInfo(fmt.Sprintf("%v", index), cm.Some(LogContext {Filename: "main.go", Line: 114}))
	
	var iter tarantoolTypes.Iterator
	empty_tuple, _ := Encode([]int{})
	result3 := tarantoolTypes.IteratorNewIterator(index, tarantoolTypes.IteratorTypeIterAll, cm.ToList(empty_tuple))
	if result2.IsOK() {
		iter = *result3.OK()
	} else {
		say.SayError(fmt.Sprintf("Iterator Error"), cm.Some(LogContext {Filename: "main.go", Line: 144}))
		return
	}

	for {
		r := iter.Next()
		if r.IsErr() {
			break
		}
		raw := *r.OK()
		say.SayInfo(fmt.Sprintf("From iter %v", raw), cm.Some(LogContext {Filename: "main.go", Line: 152}))
	}
}
