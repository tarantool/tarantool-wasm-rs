package main

import (
	"fmt"

	incominghandler "example.com/internal/wasi/http/incoming-handler"
	wasiTypes "example.com/internal/wasi/http/types"
	"example.com/internal/tarantool/tarantool/say"
	tarantoolTypes "example.com/internal/tarantool/tarantool/types"
	"go.bytecodealliance.org/cm"
)

func init() {
	incominghandler.Exports.Handle = handleRequest
}

func handleRequest(request wasiTypes.IncomingRequest, responseOut wasiTypes.ResponseOutparam) {
	fmt.Println("Incoming HTTP request!")
}


func main() {
	ctx := tarantoolTypes.LogContext{
		Filename: "go",
		Line:     1337,
	}
	say.SayInfo("GO\n", cm.Some(ctx))
}
