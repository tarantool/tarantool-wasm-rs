import sys
import time

from wit_world import exports
from wit_world.imports import box_tuple, database, transaction
from componentize_py_types import Err


class Run(exports.Run):
    def run(self) -> None:
        print("ready", flush=True)
        for line in sys.stdin:
            parts = line.split()
            if not parts:
                continue
            command = parts[0]
            try:
                if command == "BEGIN":
                    transaction.begin()
                    print("begin", flush=True)
                elif command == "ID":
                    print(f"id={transaction.id()}", flush=True)
                elif command == "INSERT":
                    # MessagePack [positive fixint], avoiding a Python dependency.
                    key = int(parts[1])
                    assert 0 <= key < 128
                    space = database.space_by_name("pipe_txn")
                    assert space is not None
                    value = database.insert(space, bytes([0x91, key]))
                    box_tuple.release(value)
                    print("insert", flush=True)
                elif command == "COMMIT":
                    transaction.commit()
                    print("commit", flush=True)
                elif command == "ROLLBACK":
                    transaction.rollback()
                    print("rollback", flush=True)
                elif command == "LOOP":
                    print("looping", flush=True)
                    while True:
                        pass
                elif command == "BUSY":
                    print("busy", flush=True)
                    deadline = time.monotonic() + 1.0
                    while time.monotonic() < deadline:
                        pass
                    print("done", flush=True)
                elif command == "SPAM":
                    while True:
                        print("x" * 8192, flush=True)
                elif command == "FINISH":
                    print("finished", flush=True)
                    return
                elif command == "TRAP":
                    raise RuntimeError("intentional guest failure")
                else:
                    raise ValueError(command)
            except Err:
                # Keep running so tests can check the transaction after an error.
                print(f"error:{command}", flush=True)
