package main

import (
    "go.bytecodealliance.org/cm"
    "adder-wasm-bindings/internal/docs/adder/adder"
    ss "adder-wasm-bindings/internal/tarantool/tarantool/session"
    "adder-wasm-bindings/internal/tarantool/tarantool/types"
)

type BoxError = types.BoxError
type Session = types.Session

func init() {

	adder.Exports.TCurrent = func() (result Session) {
        return ss.Current()
    }

	adder.Exports.TBroadcast = func(key string, value string) (result cm.Result[BoxError, struct{}, BoxError]) {
        res := ss.Broadcast(key, value)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxError, struct{}, BoxError], BoxError, struct{}, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxError, struct{}, BoxError], BoxError, struct{}, BoxError](*res.Err())
    }

	adder.Exports.TIprotoSend = func(session Session, header cm.List[uint8], body cm.List[uint8]) (result cm.Result[BoxError, struct{}, BoxError]) {
        res := ss.IprotoSend(session, header, body)
		if res.IsOK() {
			return cm.OK[cm.Result[BoxError, struct{}, BoxError], BoxError, struct{}, BoxError](*res.OK())
		}
		return cm.Err[cm.Result[BoxError, struct{}, BoxError], BoxError, struct{}, BoxError](*res.Err())
    }
}

func main() {}
