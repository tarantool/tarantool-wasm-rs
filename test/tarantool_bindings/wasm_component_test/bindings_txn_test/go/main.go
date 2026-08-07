package main

import (
	"fmt"

	"adder-wasm-bindings/internal/tarantool/tarantool/log"
	"adder-wasm-bindings/internal/tarantool/tarantool/transaction"
	"adder-wasm-bindings/internal/tarantool/tarantool/types"
	"adder-wasm-bindings/internal/wasi/cli/run"
	"go.bytecodealliance.org/cm"
)

type LogContext = types.LogContext

func init() {
	run.Exports.Run = func() (result cm.BoolResult) {
		logInfo(fmt.Sprintf("transaction id before begin: %s", optionalID(transaction.ID())))

		if begin := transaction.Begin(); begin.IsErr() {
			logInfo(fmt.Sprintf("transaction.begin() failed: %v", *begin.Err()))
			return
		}
		logInfo("transaction.begin()")
		logInfo(fmt.Sprintf("transaction id first: %s", optionalID(transaction.ID())))
		logInfo(fmt.Sprintf("transaction isolation: %s", optionalIsolation(transaction.Isolation())))

		transaction.MakeSynchronous()
		logInfo("transaction.make_synchronous()")

		if changed := transaction.SetIsolation(types.TransactionIsolationBestEffort); changed.IsErr() {
			logInfo(fmt.Sprintf("transaction.set_isolation() failed: %v", *changed.Err()))
			return
		}
		logInfo("transaction.set_isolation()")
		logInfo(fmt.Sprintf("transaction isolation after set: %s", optionalIsolation(transaction.Isolation())))

		if committed := transaction.Commit(); committed.IsErr() {
			logInfo(fmt.Sprintf("transaction.commit() failed: %v", *committed.Err()))
			return
		}
		logInfo("transaction.commit()")

		if begin := transaction.Begin(); begin.IsErr() {
			logInfo(fmt.Sprintf("second transaction.begin() failed: %v", *begin.Err()))
			return
		}
		logInfo("transaction.begin() second")
		logInfo(fmt.Sprintf("transaction id second: %s", optionalID(transaction.ID())))

		if rolledBack := transaction.Rollback(); rolledBack.IsErr() {
			logInfo(fmt.Sprintf("transaction.rollback() failed: %v", *rolledBack.Err()))
			return
		}
		logInfo("transaction.rollback()")
		logInfo(fmt.Sprintf("transaction id after rollback: %s", optionalID(transaction.ID())))
		return
	}
}

func logInfo(message string) {
	log.Write(types.LogLevelInfo, message, cm.None[LogContext]())
}

func optionalID(value cm.Option[uint64]) string {
	if id := value.Some(); id != nil {
		return fmt.Sprintf("%d", *id)
	}
	return "none"
}

func optionalIsolation(value cm.Option[types.TransactionIsolation]) string {
	if isolation := value.Some(); isolation != nil {
		return isolation.String()
	}
	return "none"
}

func main() {}
