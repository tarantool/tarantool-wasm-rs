from wit_world.imports import types, ttbox, box_tuple

class WitWorld:

    def t_schema_version(self) -> int:
        return ttbox.schema_version()
        
    def t_space_by_name(self, name: str) -> types.Space:
        return ttbox.space_by_name(name)
    
    def t_index_by_name(self, space: types.Space, name: str) -> types.Index:
        return ttbox.index_by_name(space, name)

    def t_insert(self, space: types.Space, tup: bytes) -> bytes:
        return box_tuple.to_buf(ttbox.insert(space, tup))

    def t_update(self, index: types.Index, key: bytes, ops: bytes) -> bytes:
        return box_tuple.to_buf(ttbox.update(index, key, ops))

    def t_replace(self, space: types.Space, tup: bytes) -> bytes:
        return box_tuple.to_buf(ttbox.replace(space, tup))

    def t_upsert(self, index: types.Index, tup: bytes, ops: bytes) -> None:
        ttbox.upsert(index, tup, ops)
        
    def t_delete(self, index: types.Index, key: bytes) -> bytes:
        return box_tuple.to_buf(ttbox.delete(index, key))
        
    def t_truncate(self, space: types.Space) -> None:
        ttbox.truncate(space)
