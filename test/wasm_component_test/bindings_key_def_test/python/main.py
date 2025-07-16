from wit_world.imports import types, key_def as kd, msgpack, box_tuple
from typing import List, Tuple

class WitWorld:

    def t_new(self, parts: List[types.KeyPart]) -> int:
        return kd.new(parts)

    def t_dup(self, key_def: int) -> int:
        return kd.dup(key_def)

    def t_merge(self, left: int, right: int) -> int:
        return kd.merge(left, right)

    def t_delete(self, key_def: int) -> None:
        kd.delete(key_def)

    def t_dump_parts(self, key_def: int) -> List[types.KeyPart]:
        return kd.dump_parts(key_def)

    def t_validate_key(self, key_def: int, key: bytes) -> Tuple[bool, int]:
        return kd.validate_key(key_def, key)

    def t_validate_full_key(self, key_def: int, key: bytes) -> Tuple[bool, int]:
        return kd.validate_full_key(key_def, key)

    def t_part_count(self, key_def: int) -> int:
        return kd.part_count(key_def)

    def t_extract_key(self, key_def: int, tuple: int) -> bytes:
        return kd.extract_key(key_def, tuple)

    def t_validate_tuple(self, key_def: int, tuple: int) -> bool:
        return kd.validate_tuple(key_def, tuple)

    def decode_from_raw_ptr(self, ptr: int) -> bytes:
        return msgpack.decode_from_raw_ptr(ptr)

    def t_tuple_new(self, data: bytes) -> int:
        return box_tuple.new(data)
