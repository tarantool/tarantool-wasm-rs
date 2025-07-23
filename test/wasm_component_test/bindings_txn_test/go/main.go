package main

import (
	"fmt"

	"adder-wasm-bindings/internal/tarantool/tarantool/say"
	"adder-wasm-bindings/internal/tarantool/tarantool/txn"
	"adder-wasm-bindings/internal/tarantool/tarantool/types"
	"adder-wasm-bindings/internal/wasi/cli/run"

	"go.bytecodealliance.org/cm"
)

type LogContext = types.LogContext

func init() {
	run.Exports.Run = func() (result cm.BoolResult) {
		logInfo(fmt.Sprintf("txn() before begin: %s", boolStr(txn.Txn())))

		// txn.begin()
		resBegin := txn.Begin()
		if resBegin.IsOK() {
			logInfo("txn.begin()")
		} else {
			logInfo(fmt.Sprintf("txn.begin() failed: %v", *resBegin.Err()))
			return
		}

		txnID := txn.TxnID()
		isolation := txn.TxnIsolation()

		logInfo(fmt.Sprintf("txn_id first: %d", txnID))
		logInfo(fmt.Sprintf("txn_isolation: %s", isolationToStr(isolation)))

		txn.TxnMakeSync()
		logInfo("txn_make_sync()")

		resIso := txn.TxnSetIsolation(types.TxnIsolationLevelTxnIsolationBestEffort)
		if resIso.IsOK() {
			logInfo("txn_set_isolation()")
		} else {
			logInfo(fmt.Sprintf("txn_set_isolation failed: %v", *resIso.Err()))
			return
		}

		newIso := txn.TxnIsolation()
		logInfo(fmt.Sprintf("txn_isolation after set: %s", isolationToStr(newIso)))

		resCommit := txn.Commit()
		if resCommit.IsOK() {
			logInfo("txn.commit()")
		} else {
			logInfo(fmt.Sprintf("txn.commit() failed: %v", *resCommit.Err()))
			return
		}

		resBegin2 := txn.Begin()
		if resBegin2.IsOK() {
			logInfo("txn.begin() second")
		} else {
			logInfo(fmt.Sprintf("second txn.begin() failed: %v", *resBegin2.Err()))
			return
		}
        txnID = txn.TxnID()
		logInfo(fmt.Sprintf("txn_id second: %d", txnID))

		// rollback
		resRollback := txn.Rollback()
		if resRollback.IsOK() {
			logInfo("txn.rollback()")
		} else {
			logInfo(fmt.Sprintf("txn.rollback() failed: %v", *resRollback.Err()))
			return
		}

		logInfo(fmt.Sprintf("txn() after rollback: %s", boolStr(txn.Txn())))
		return
	}
}

func main() {}

// --- helpers ---

func logInfo(msg string) {
	say.SayInfo(msg, cm.None[LogContext]())
}

func boolStr(b bool) string {
	if b {
		return "True"
	}
	return "False"
}

func isolationToStr(level types.TxnIsolationLevel) string {
	switch level {
	case types.TxnIsolationLevelTxnIsolationDefault:
		return "TXN_ISOLATION_DEFAULT"
	case types.TxnIsolationLevelTxnIsolationBestEffort:
		return "TXN_ISOLATION_BEST_EFFORT"
	case types.TxnIsolationLevelTxnIsolationReadCommitted:
		return "TXN_ISOLATION_READ_COMMITTED"
	case types.TxnIsolationLevelTxnIsolationReadConfirmed:
		return "TXN_ISOLATION_READ_CONFIRMED"
	case types.TxnIsolationLevelTxnIsolationLinearizable:
		return "TXN_ISOLATION_LINEARIZABLE"
	default:
		return fmt.Sprintf("UNKNOWN(%d)", level)
	}
}
