from wit_world import exports
from wit_world.imports import ttbox as box, box_tuple, msgpack
from wit_world.imports.types import Space, Index, Iterator, IteratorType

import json

class Run(exports.Run):
     def run(self) -> None:
        try:
            space: Space = box.space_by_name("tester")
            index: Index = box.index_by_name(space, "primary")
            iterator = Iterator.new_iterator(index, IteratorType.ITER_ALL, msgpack.encode(json.dumps([]).encode('utf-8')))
            while True:
                try:
                    tup = iterator.next()
                    data = box_tuple.to_buf(tup)
                    decoded_json_bytes: bytes = msgpack.decode(data)
                    decoded_json_string = decoded_json_bytes.decode('utf-8')
                    print(decoded_json_string)
                except Exception:
                    # End of iteration (StopIteration or similar)
                    break

        except Exception as e:
            print(e)
