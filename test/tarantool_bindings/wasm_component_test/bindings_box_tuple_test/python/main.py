from wit_world.imports import msgpack, box_tuple, tuple_format, key_def, types
from typing import List
import json

class WitWorld:
    def t_new(self, data: bytes) -> types.BoxTuple:
        return box_tuple.new(data)

    def t_ref(self, t: types.BoxTuple) -> int:
        return box_tuple.ref(t)

    def t_unref(self, t: types.BoxTuple) -> None:
        return box_tuple.unref(t)

    def t_field_count(self, t: types.BoxTuple) -> int:
        return box_tuple.field_count(t)

    def t_bsize(self, t: types.BoxTuple) -> int:
        return box_tuple.bsize(t)

    def t_to_buf(self, t: types.BoxTuple) -> bytes:
        return box_tuple.to_buf(t)

    def t_format(self, t: types.BoxTuple) -> types.TupleFormat:
        return box_tuple.format(t)

    def t_field(self, t: types.BoxTuple, idx: int) -> types.TupleField:
        return box_tuple.field(t, idx)

    def t_field_by_path(self, t: types.BoxTuple, path: str, index_base: int) -> types.TupleField:
        return box_tuple.field_by_path(t, path, index_base)

    def t_update(self, t: types.BoxTuple, expr: bytes) -> types.BoxTuple:
        return box_tuple.update(t, expr)

    def t_upsert(self, t: types.BoxTuple, expr: bytes) -> types.BoxTuple:
        return box_tuple.upsert(t, expr)

    def t_validate(self, t: types.BoxTuple, format: types.TupleFormat) -> bool:
        return box_tuple.validate(t, format)

    def t_compare(self, key_def_: types.KeyDef, left: types.BoxTuple, right: types.BoxTuple) -> int:
        return box_tuple.compare(key_def_, left, right)

    def t_compare_with_key(self, tuple: types.BoxTuple, key: bytes, key_def_: types.KeyDef) -> int:
        return box_tuple.compare_with_key(tuple, key, key_def_)

    def t_key_def_new(self, parts: List[types.KeyPart]) -> types.KeyDef:
        return key_def.new(parts)

    def t_tuple_format_default(self) -> types.TupleFormat:
        return tuple_format.default()

    def t_tuple_format_new(self, key_defs: List[types.KeyDef]) -> types.TupleFormat:
        return tuple_format.new(key_defs)

    def t_decode_from_raw_ptr(self, ptr: int) -> bytes:
        return msgpack.decode_from_raw_ptr(ptr)

    def t_iter_test_all(self, t: types.BoxTuple) -> bytes:
        it = box_tuple.TupleIterator.new_tuple_iterator(t)
        result = []

        while True:
            pos = it.position()
            field = it.next()
            if field.ptr == 0:
                break
            result.append([pos, field.ptr])

        it.rewind()
        pos0 = it.position()
        field0 = it.next()
        result.append(["rewind", pos0, field0.ptr])

        field1 = it.seek(1)
        pos1 = it.position()
        result.append(["seek", pos1, field1.ptr])

        return msgpack.encode(json.dumps(result).encode("utf-8"))
