import sys
import time

from wit_world import exports
from wit_world.imports import box_tuple, database, transaction, experimental_read, index as index_api, tarantool_tarantool_types
from componentize_py_types import Err


class Run(exports.Run):
    def run(self) -> None:
        read_ctx = None
        read_iter = None
        normal_iter = None
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
                elif command == "READ_OPEN":
                    read_ctx = experimental_read.open()
                    print("read-open", flush=True)
                elif command == "READ_CLOSE":
                    assert read_ctx is not None
                    read_ctx.close()
                    print("read-close", flush=True)
                elif command == "READ_COUNT":
                    assert read_ctx is not None
                    name = parts[1] if len(parts) > 1 else "pipe_txn"
                    space = experimental_read.space_by_name(read_ctx, name)
                    assert space is not None
                    index = experimental_read.index_by_name(read_ctx, space, "primary")
                    assert index is not None
                    count = experimental_read.count(
                        read_ctx, index, tarantool_tarantool_types.IteratorType.ALL, bytes([0x90]))
                    print(f"count={count}", flush=True)
                elif command == "READ_SELECT":
                    assert read_ctx is not None
                    name = parts[1] if len(parts) > 1 else "pipe_txn"
                    space = experimental_read.space_by_name(read_ctx, name)
                    assert space is not None
                    index = experimental_read.index_by_name(read_ctx, space, "primary")
                    assert index is not None
                    read_iter = experimental_read.select(
                        read_ctx, index, tarantool_tarantool_types.IteratorType.ALL, bytes([0x90]))
                    print("read-select", flush=True)
                elif command in ("READ_NEXT", "READ_NEXT_KEY"):
                    assert read_iter is not None
                    value = read_iter.next(read_ctx)
                    key = None
                    if value is not None:
                        if command == "READ_NEXT_KEY":
                            encoded = box_tuple.to_bytes(value)
                            assert 0x90 <= encoded[0] <= 0x9f and encoded[1] < 128
                            key = encoded[1]
                        box_tuple.release(value)
                    if command == "READ_NEXT_KEY":
                        print(f"key={key}" if key is not None else "end", flush=True)
                    else:
                        print("found" if value is not None else "end", flush=True)
                elif command == "INDEX_OPEN":
                    space = database.space_by_name("pipe_txn")
                    assert space is not None
                    idx = database.index_by_name(space, "primary")
                    assert idx is not None
                    normal_iter = index_api.Iterator.new(
                        idx, tarantool_tarantool_types.IteratorType.ALL, bytes([0x90]))
                    print("index-open", flush=True)
                elif command == "INDEX_NEXT":
                    assert normal_iter is not None
                    value = normal_iter.next()
                    if value is not None:
                        box_tuple.release(value)
                    print("found" if value is not None else "end", flush=True)
                elif command == "INDEX_CLOSE":
                    assert normal_iter is not None
                    normal_iter.__exit__(None, None, None)
                    normal_iter = None
                    print("index-close", flush=True)
                elif command == "READ_GET":
                    assert read_ctx is not None
                    name = parts[1] if len(parts) > 2 else "pipe_txn"
                    key = int(parts[-1])
                    assert 0 <= key < 128
                    space = experimental_read.space_by_name(read_ctx, name)
                    assert space is not None
                    index = experimental_read.index_by_name(read_ctx, space, "primary")
                    assert index is not None
                    value = experimental_read.get(read_ctx, index, bytes([0x91, key]))
                    if value is not None:
                        box_tuple.release(value)
                    print("found" if value is not None else "missing", flush=True)
                elif command == "READ_BAD_KEY":
                    assert read_ctx is not None
                    space = experimental_read.space_by_name(read_ctx, parts[1])
                    assert space is not None
                    index = experimental_read.index_by_name(read_ctx, space, "primary")
                    assert index is not None
                    experimental_read.get(read_ctx, index, bytes([0xc0]))
                    print("unexpected-success", flush=True)
                elif command == "TX_GET":
                    key = int(parts[1])
                    assert 0 <= key < 128
                    space = database.space_by_name("pipe_txn")
                    assert space is not None
                    idx = database.index_by_name(space, "primary")
                    assert idx is not None
                    value = index_api.get(idx, bytes([0x91, key]))
                    if value is not None:
                        box_tuple.release(value)
                    print("found" if value is not None else "missing", flush=True)
                elif command == "ID":
                    print(f"id={transaction.id()}", flush=True)
                elif command == "SPACE_LOOKUP":
                    found = database.space_by_name("pipe_txn")
                    print("space-found" if found is not None else "space-missing", flush=True)
                elif command == "INSERT":
                    # MessagePack [positive fixint], avoiding a Python dependency.
                    name = parts[1] if len(parts) > 2 else "pipe_txn"
                    key = int(parts[-1])
                    assert 0 <= key < 128
                    space = database.space_by_name(name)
                    assert space is not None
                    value = database.insert(space, bytes([0x91, key]))
                    box_tuple.release(value)
                    print("insert", flush=True)
                elif command == "REPLACE":
                    name = parts[1] if len(parts) > 2 else "pipe_txn"
                    key = int(parts[-1])
                    assert 0 <= key < 128
                    space = database.space_by_name(name)
                    assert space is not None
                    value = database.replace(space, bytes([0x91, key]))
                    box_tuple.release(value)
                    print("replace", flush=True)
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
            except Err as err:
                # Keep running so tests can check the transaction after an error.
                if command == "READ_OPEN" and "snapshot unavailable:" in err.value.message:
                    print("error:READ_OPEN:snapshot-unavailable", flush=True)
                elif command in ("SPACE_LOOKUP", "READ_NEXT", "INDEX_NEXT"):
                    print(f"error:{command}:{err.value.code}", flush=True)
                else:
                    print(f"error:{command}", flush=True)
