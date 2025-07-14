from wit_world.imports import types, index as idx, box_tuple

class WitWorld():

    def t_len(self, index: types.Index) -> int:
        return idx.len(index)

    def t_bsize(self, index: types.Index) -> int:
        return idx.bsize(index)

    def t_random(self, index: types.Index, rnd: int) -> bytes:
        ptr = idx.random(index, rnd)
        return box_tuple.to_buf(ptr)

    def t_get(self, index: types.Index, key: bytes) -> bytes:
        ptr = idx.get(index, key)
        return box_tuple.to_buf(ptr)

    def t_min(self, index: types.Index, key: bytes) -> bytes:
        ptr = idx.min(index, key)
        return box_tuple.to_buf(ptr)

    def t_max(self, index: types.Index, key: bytes) -> bytes:
        ptr = idx.max(index, key)
        return box_tuple.to_buf(ptr)

    def t_count(self, index: types.Index, iter_type: types.IteratorType, key: bytes) -> int:
        return idx.count(index, iter_type, key)
