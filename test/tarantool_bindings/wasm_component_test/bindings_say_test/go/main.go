package main

import (
	"adder-wasm-bindings/internal/docs/adder/adder"
	"adder-wasm-bindings/internal/tarantool/tarantool/log"
	"adder-wasm-bindings/internal/tarantool/tarantool/types"
	"go.bytecodealliance.org/cm"
)

type LogLevel = types.LogLevel
type LogContext = types.LogContext

func init() {
	adder.Exports.TSay = log.Write
	adder.Exports.TSayError = writeAt(types.LogLevelError)
	adder.Exports.TSayCrit = writeAt(types.LogLevelCritical)
	adder.Exports.TSayWarn = writeAt(types.LogLevelWarning)
	adder.Exports.TSayInfo = writeAt(types.LogLevelInfo)
	adder.Exports.TSayVerbose = writeAt(types.LogLevelVerbose)
	adder.Exports.TSayDebug = writeAt(types.LogLevelDebug)
}

func writeAt(level LogLevel) func(string, cm.Option[LogContext]) {
	return func(message string, context cm.Option[LogContext]) {
		log.Write(level, message, context)
	}
}

func main() {}
