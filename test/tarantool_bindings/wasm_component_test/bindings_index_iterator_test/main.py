from wit_world import exports
from wit_world.imports import database as box, index, box_tuple, msgpack
from wit_world.imports.types import Space, Index, IteratorType

import json

class Run(exports.Run):
     def run(self) -> None:
        try:
            space: Space = box.space_by_name("tester")
            idx: Index = box.index_by_name(space, "primary")
            key = msgpack.from_json(json.dumps([]))
            with index.Iterator.new(idx, IteratorType.ALL, key) as iterator:
                while True:
                    tup = iterator.next()
                    if tup is None:
                        break
                    try:
                        data = box_tuple.to_bytes(tup)
                    finally:
                        box_tuple.release(tup)
                    print(msgpack.to_json(data))

        except Exception as e:
            print(e)
