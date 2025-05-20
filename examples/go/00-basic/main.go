package main

import (
	"fmt"

	incominghandler "adder-wasm-bindings/internal/wasi/http/incoming-handler"
	wasiTypes "adder-wasm-bindings/internal/wasi/http/types"
)

func init() {
	incominghandler.Exports.Handle = handleRequest
}

func handleRequest(request wasiTypes.IncomingRequest, responseOut wasiTypes.ResponseOutparam) {
	fmt.Println("Incoming HTTP request!")
}

func main() {
	fmt.Println("Hello from Go WASM!")
}
