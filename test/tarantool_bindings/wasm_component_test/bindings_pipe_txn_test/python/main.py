import sys

from wit_world import exports
from wit_world.imports import transaction


class Run(exports.Run):
    def run(self) -> None:
        # The host writes each command as its own pipe message, separated in
        # time. The transaction stays open across the reads, which is exactly
        # the scenario that used to park Tarantool's TX thread.
        print("ready", flush=True)

        while True:
            line = sys.stdin.readline()
            if not line:
                # stdin closed without an explicit end; the transaction fiber
                # rolls back anything still open.
                return
            command = line.strip()
            if command == "BEGIN":
                transaction.begin()
                print("begin", flush=True)
            elif command == "ID":
                print(f"id={transaction.id()}", flush=True)
            elif command == "COMMIT":
                transaction.commit()
                print("commit", flush=True)
                return
            elif command == "ROLLBACK":
                transaction.rollback()
                print("rollback", flush=True)
                return
            else:
                print(f"unknown:{command}", flush=True)
