from typing import Optional

from wit_world.imports import box_tuple, index as index_api, types


def optional_tuple_to_bytes(value: Optional[types.BoxTuple]) -> Optional[bytes]:
    if value is None:
        return None
    try:
        return box_tuple.to_bytes(value)
    finally:
        box_tuple.release(value)


class WitWorld:
    def t_len(self, index: types.Index) -> int:
        return index_api.tuple_count(index)

    def t_bsize(self, index: types.Index) -> int:
        return index_api.memory_size(index)

    def t_random(self, index: types.Index, rnd: int) -> Optional[bytes]:
        return optional_tuple_to_bytes(index_api.random(index, rnd))

    def t_get(self, index: types.Index, key: bytes) -> Optional[bytes]:
        return optional_tuple_to_bytes(index_api.get(index, key))

    def t_min(self, index: types.Index, key: bytes) -> Optional[bytes]:
        return optional_tuple_to_bytes(index_api.min(index, key))

    def t_max(self, index: types.Index, key: bytes) -> Optional[bytes]:
        return optional_tuple_to_bytes(index_api.max(index, key))

    def t_count(
        self, index: types.Index, iter_type: types.IteratorType, key: bytes
    ) -> int:
        return index_api.count(index, iter_type, key)
