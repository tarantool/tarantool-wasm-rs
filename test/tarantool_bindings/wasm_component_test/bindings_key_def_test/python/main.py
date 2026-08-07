from typing import List

from wit_world.imports import box_tuple, key_def as kd, types


class WitWorld:
    def t_tuple_new(self, data: bytes) -> types.BoxTuple:
        return box_tuple.new(data)

    def t_new(self, parts: List[types.KeyPart]) -> types.KeyDef:
        return kd.new(parts)

    def t_duplicate(self, key_def: types.KeyDef) -> types.KeyDef:
        return kd.duplicate(key_def)

    def t_merge(self, left: types.KeyDef, right: types.KeyDef) -> types.KeyDef:
        return kd.merge(left, right)

    def t_release(self, key_def: types.KeyDef) -> None:
        kd.release(key_def)

    def t_dump_parts(self, key_def: types.KeyDef) -> List[types.KeyPart]:
        return kd.dump_parts(key_def)

    def t_validate_key(self, key_def: types.KeyDef, key: bytes) -> None:
        kd.validate_key(key_def, key)

    def t_validate_full_key(self, key_def: types.KeyDef, key: bytes) -> None:
        kd.validate_full_key(key_def, key)

    def t_part_count(self, key_def: types.KeyDef) -> int:
        return kd.part_count(key_def)

    def t_extract_key(self, key_def: types.KeyDef, tuple: types.BoxTuple) -> bytes:
        return kd.extract_key(key_def, tuple)

    def t_validate_tuple(self, key_def: types.KeyDef, tuple: types.BoxTuple) -> None:
        kd.validate_tuple(key_def, tuple)
