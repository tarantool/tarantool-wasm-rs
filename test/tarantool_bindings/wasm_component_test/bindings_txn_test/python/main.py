from wit_world import exports
from wit_world.imports import log, transaction, types


ISOLATION_NAMES = {
    types.TransactionIsolation.DEFAULT: "default",
    types.TransactionIsolation.BEST_EFFORT: "best-effort",
    types.TransactionIsolation.READ_COMMITTED: "read-committed",
    types.TransactionIsolation.READ_CONFIRMED: "read-confirmed",
    types.TransactionIsolation.LINEARIZABLE: "linearizable",
}


def info(message: str) -> None:
    log.write(types.LogLevel.INFO, message, None)


def format_transaction_id(transaction_id: int | None) -> str:
    return "none" if transaction_id is None else str(transaction_id)


class Run(exports.Run):
    def run(self) -> None:
        info(f"transaction id before begin: {format_transaction_id(transaction.id())}")

        transaction.begin()
        info("transaction.begin()")

        transaction_id = transaction.id()
        isolation = transaction.isolation()
        info(f"transaction id first: {transaction_id}")
        info(f"transaction isolation: {ISOLATION_NAMES[isolation]}")

        transaction.make_synchronous()
        info("transaction.make_synchronous()")

        transaction.set_isolation(types.TransactionIsolation.BEST_EFFORT)
        info("transaction.set_isolation()")
        new_isolation = transaction.isolation()
        info(f"transaction isolation after set: {ISOLATION_NAMES[new_isolation]}")

        transaction.commit()
        info("transaction.commit()")

        transaction.begin()
        info("transaction.begin() second")
        info(f"transaction id second: {transaction.id()}")
        transaction.rollback()
        info("transaction.rollback()")

        info(f"transaction id after rollback: {format_transaction_id(transaction.id())}")
