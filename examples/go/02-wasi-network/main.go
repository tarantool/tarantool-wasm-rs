package main

import (
	"fmt"

	incominghandler "adder-wasm-bindings/internal/wasi/http/incoming-handler"
	"adder-wasm-bindings/internal/wasi/cli/run"
	wasiTypes "adder-wasm-bindings/internal/wasi/http/types"
	"go.bytecodealliance.org/cm"

	instNet "adder-wasm-bindings/internal/wasi/sockets/instance-network"
	wasiNet "adder-wasm-bindings/internal/wasi/sockets/network"
	tcpSocket "adder-wasm-bindings/internal/wasi/sockets/tcp-create-socket"
	wasiStreams "adder-wasm-bindings/internal/wasi/io/streams"
	"adder-wasm-bindings/internal/wasi/io/poll"
	
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

type InputStream wasiStreams.InputStream
type OutputStream wasiStreams.OutputStream

func main() {
	net := instNet.InstanceNetwork()
	result := tcpSocket.CreateTCPSocket(wasiNet.IPAddressFamilyIPv4)
	if result.IsErr() {
		return
	}
	sock := *result.OK()
	pollable := sock.Subscribe()
	pollables := []poll.Pollable {pollable}

	peer := wasiNet.IPv4SocketAddress{
		Port: 12121,
		Address: wasiNet.IPv4Address{127, 0, 0, 1},
	}
	sock.StartConnect(net, wasiNet.IPSocketAddressIPv4(peer))

	for {
		readyHandles := poll.Poll(cm.ToList(pollables)).Slice()
		if len(readyHandles) > 0 {
			connResult := sock.FinishConnect()
			if connResult.IsErr() {
				fmt.Println("Connection failed")
				return
			}

			inStream := connResult.OK().F0
			outStream := connResult.OK().F1

			message := "hello from go wasm\n"
			outStream.BlockingWriteAndFlush(cm.ToList([]uint8(message)))

			msgRes := inStream.BlockingRead(1024)
			if msgRes.IsOK() {
				msg := *msgRes.OK()
				fmt.Printf("Msg from server: %s\n", string(msg.Slice()))
			}

			sock.Shutdown(2)
			break
		} else {
			pollable.Block()
		}
	}
}
