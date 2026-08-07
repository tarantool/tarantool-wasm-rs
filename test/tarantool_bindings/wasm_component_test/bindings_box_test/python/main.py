from typing import Optional

from wit_world.imports import box_tuple, database, types


def tuple_to_bytes(value: types.BoxTuple) -> bytes:
    try:
        return box_tuple.to_bytes(value)
    finally:
        box_tuple.release(value)


def optional_tuple_to_bytes(value: Optional[types.BoxTuple]) -> Optional[bytes]:
    return None if value is None else tuple_to_bytes(value)


class WitWorld:
    def t_schema_version(self) -> int:
        return database.schema_version()

    def t_space_by_name(self, name: str) -> Optional[types.Space]:
        return database.space_by_name(name)

    def t_index_by_name(
        self, space: types.Space, name: str
    ) -> Optional[types.Index]:
        return database.index_by_name(space, name)

    def t_insert(self, space: types.Space, value: bytes) -> bytes:
        return tuple_to_bytes(database.insert(space, value))

    def t_update(
        self, index: types.Index, key: bytes, operations: bytes
    ) -> Optional[bytes]:
        return optional_tuple_to_bytes(database.update(index, key, operations))

    def t_replace(self, space: types.Space, value: bytes) -> bytes:
        return tuple_to_bytes(database.replace(space, value))

    def t_upsert(
        self, index: types.Index, value: bytes, operations: bytes
    ) -> None:
        database.upsert(index, value, operations)

    def t_delete(self, index: types.Index, key: bytes) -> Optional[bytes]:
        return optional_tuple_to_bytes(database.delete(index, key))

    def t_truncate(self, space: types.Space) -> None:
        database.truncate(space)
