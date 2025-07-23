from wit_world.imports import types, txn, say
from wit_world import exports

d = {
    types.TxnIsolationLevel.TXN_ISOLATION_DEFAULT: "TXN_ISOLATION_DEFAULT",
    types.TxnIsolationLevel.TXN_ISOLATION_BEST_EFFORT: "TXN_ISOLATION_BEST_EFFORT",
    types.TxnIsolationLevel.TXN_ISOLATION_READ_COMMITTED: "TXN_ISOLATION_READ_COMMITTED",
    types.TxnIsolationLevel.TXN_ISOLATION_READ_CONFIRMED: "TXN_ISOLATION_READ_CONFIRMED",
    types.TxnIsolationLevel.TXN_ISOLATION_LINEARIZABLE: "TXN_ISOLATION_LINEARIZABLE",
}

class Run(exports.Run):
    def run(self) -> None:
        say.say_info(f"txn() before begin: {txn.txn()}", None)

        txn.begin()
        say.say_info("txn.begin()", None)

        txn_id = txn.txn_id()
        isolation = txn.txn_isolation()
        say.say_info(f"txn_id first: {txn_id}", None)
        say.say_info(f"txn_isolation: {d[isolation]}", None)

        txn.txn_make_sync()
        say.say_info("txn_make_sync()", None)

        txn.txn_set_isolation(types.TxnIsolationLevel.TXN_ISOLATION_BEST_EFFORT)
        new_isolation = txn.txn_isolation()
        say.say_info(f"txn_isolation after set: {d[new_isolation]}", None)

        txn.commit()
        say.say_info("txn.commit()", None)

        txn.begin()
        say.say_info("txn.begin() second", None)
        say.say_info(f"txn_id second: {txn_id}", None)
        txn.rollback()
        say.say_info("txn.rollback()", None)

        say.say_info(f"txn() after rollback: {txn.txn()}", None)
