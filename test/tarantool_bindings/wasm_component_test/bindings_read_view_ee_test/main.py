import json

from wit_world import exports
from wit_world.imports import box_tuple, build_info, database, msgpack, read_view
from wit_world.imports.types import IteratorType

SPACE_ID = 777
INDEX_ID = 0


def encode(value: object) -> bytes:
    return msgpack.from_json(json.dumps(value, separators=(",", ":")))


def decode(value: bytes | None) -> object | None:
    return None if value is None else json.loads(msgpack.to_json(value))


class Run(exports.Run):
    def run(self) -> None:
        edition = build_info.current_edition()
        print(edition.name.lower())
        if edition == build_info.Edition.COMMUNITY:
            try:
                read_view.open("unsupported-read-view")
            except Exception as error:
                expected = "raw read-view API requires Tarantool Enterprise Edition"
                if expected not in str(error):
                    raise RuntimeError(f"unexpected read-view error: {error}") from error
                print(expected)
                return
            raise RuntimeError("read-view unexpectedly opened on Community Edition")

        space = database.space_by_name("read_view_wasm_test")
        if space is None:
            raise RuntimeError("test space is missing")

        view = read_view.open("wasm-read-view-test")
        try:
            print(json.dumps(decode(view.get(SPACE_ID, INDEX_ID, encode([1])))))

            replacement = database.replace(space, encode([1, "new"]))
            box_tuple.release(replacement)
            inserted = database.insert(space, encode([3, "c"]))
            box_tuple.release(inserted)

            print(json.dumps(decode(view.get(SPACE_ID, INDEX_ID, encode([1])))))
            print(view.count(SPACE_ID, INDEX_ID, IteratorType.ALL, encode([])))

            rows = []
            iterator = view.iterator(SPACE_ID, INDEX_ID, IteratorType.ALL, encode([]))
            view.__exit__(None, None, None)
            view = None
            try:
                while True:
                    row = iterator.next()
                    if row is None:
                        break
                    rows.append(decode(row))
            finally:
                iterator.__exit__(None, None, None)
            print(json.dumps(rows, separators=(",", ":")))
        finally:
            if view is not None:
                view.__exit__(None, None, None)
