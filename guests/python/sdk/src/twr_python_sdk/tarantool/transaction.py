"""Synchronous transactions in the current guest execution context."""

from .types import TransactionIsolation


class Transactions:
    def __init__(self, bindings):
        self._bindings = bindings

    @property
    def id(self):
        return self._bindings.call("transaction", "id")

    @property
    def isolation(self):
        value = self._bindings.call("transaction", "isolation")
        return (
            None
            if value is None
            else TransactionIsolation(value.name.lower().replace("_", "-"))
        )

    def begin(self, *, isolation=None, synchronous=False):
        if self.id is not None:
            raise RuntimeError(
                "a transaction is already active; nested transactions are unsupported"
            )
        converted = (
            None
            if isolation is None
            else self._bindings.enum(
                "TransactionIsolation", TransactionIsolation(isolation)
            )
        )
        self._bindings.call("transaction", "begin")
        try:
            if converted is not None:
                self._bindings.call("transaction", "set_isolation", converted)
            if synchronous:
                self.make_synchronous()
        except Exception:
            try:
                self.rollback()
            except Exception:
                pass
            raise

    def commit(self):
        self._bindings.call("transaction", "commit")

    def rollback(self):
        self._bindings.call("transaction", "rollback")

    def set_isolation(self, isolation):
        self._bindings.call(
            "transaction",
            "set_isolation",
            self._bindings.enum(
                "TransactionIsolation", TransactionIsolation(isolation)
            ),
        )

    def make_synchronous(self):
        self._bindings.call("transaction", "make_synchronous")

    def __call__(self, *, isolation=None, synchronous=False):
        return Transaction(self, isolation, synchronous)


class Transaction:
    def __init__(self, transactions, isolation, synchronous):
        self._transactions = transactions
        self._isolation = isolation
        self._synchronous = synchronous
        self._active = False
        self._id = None

    def __enter__(self):
        if self._active:
            raise RuntimeError("transaction context is already active")
        self._transactions.begin(
            isolation=self._isolation, synchronous=self._synchronous
        )
        self._id = self._transactions.id
        self._active = True
        return self

    def __exit__(self, exc_type, exc, traceback):
        if not self._active:
            return False
        self._active = False
        if self._transactions.id != self._id:
            if exc is None:
                raise RuntimeError(
                    "transaction was ended or replaced inside its context"
                )
            return False
        if exc is None:
            # The host tears down the transaction fiber even if commit fails.
            self._transactions.commit()
        else:
            try:
                self._transactions.rollback()
            except Exception:
                pass
        return False
