package main

import (
	"fmt"

	incominghandler "adder-wasm-bindings/internal/wasi/http/incoming-handler"
	"adder-wasm-bindings/internal/wasi/cli/run"
	wasiTypes "adder-wasm-bindings/internal/wasi/http/types"
	"go.bytecodealliance.org/cm"
)

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

func main() {
	fmt.Println("Hello from Go WASM!")
}
