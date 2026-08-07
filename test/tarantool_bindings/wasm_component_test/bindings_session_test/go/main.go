package main

import (
	"adder-wasm-bindings/internal/docs/adder/adder"
	ss "adder-wasm-bindings/internal/tarantool/tarantool/session"
	"adder-wasm-bindings/internal/tarantool/tarantool/types"
	"go.bytecodealliance.org/cm"
)

type Session = types.Session
type MsgpackValue = types.MsgpackValue

func init() {
	adder.Exports.TCurrent = ss.Current
	adder.Exports.TBroadcast = ss.Broadcast
	adder.Exports.TIprotoSend = func(session Session, header MsgpackValue, body cm.Option[MsgpackValue]) cm.Result[types.BoxError, struct{}, types.BoxError] {
		return ss.IprotoSend(session, header, body)
	}
}

func main() {}
