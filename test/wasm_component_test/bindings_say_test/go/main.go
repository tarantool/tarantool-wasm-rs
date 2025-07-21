package main

import (
    "go.bytecodealliance.org/cm"
    "adder-wasm-bindings/internal/docs/adder/adder"
    "adder-wasm-bindings/internal/tarantool/tarantool/say"
    "adder-wasm-bindings/internal/tarantool/tarantool/types"

)

type LogLevel = types.LogLevel
type LogContext = adder.LogContext

func init() {
    adder.Exports.TSay = func(level LogLevel, msg string, ctx cm.Option[LogContext]) {
        say.Say(level, msg, ctx)
    }
    
    adder.Exports.TSayError = func(msg string, ctx cm.Option[LogContext]) {
        say.SayError(msg, ctx)
    }

    adder.Exports.TSayWarn = func(msg string, ctx cm.Option[LogContext]) {
        say.SayWarn(msg, ctx)
    }

    adder.Exports.TSayDebug = func(msg string, ctx cm.Option[LogContext]) {
        say.SayDebug(msg, ctx)
    }

    adder.Exports.TSayCrit = func(msg string, ctx cm.Option[LogContext]) {
        say.SayCrit(msg, ctx)
    }

    adder.Exports.TSayInfo = func(msg string, ctx cm.Option[LogContext]) {
        say.SayInfo(msg, ctx)
    }

    adder.Exports.TSayVerbose = func(msg string, ctx cm.Option[LogContext]) {
        say.SayVerbose(msg, ctx)
    }

    adder.Exports.TSaySyserror = func(msg string, ctx cm.Option[LogContext]) {
        say.SaySyserror(msg, ctx)
    }
}

func main() {}
