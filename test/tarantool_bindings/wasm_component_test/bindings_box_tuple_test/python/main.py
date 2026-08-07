import json
from typing import List, Optional

from wit_world.imports import box_tuple, key_def, msgpack, tuple_format
from wit_world.imports import tarantool_tarantool_types as types


class WitWorld:
    def t_new(self, data: bytes) -> types.BoxTuple:
        return box_tuple.new(data)

    def t_retain(self, t: types.BoxTuple) -> None:
        box_tuple.retain(t)

    def t_release(self, t: types.BoxTuple) -> None:
        box_tuple.release(t)

    def t_field_count(self, t: types.BoxTuple) -> int:
        return box_tuple.field_count(t)

    def t_byte_size(self, t: types.BoxTuple) -> int:
        return box_tuple.byte_size(t)

    def t_to_bytes(self, t: types.BoxTuple) -> bytes:
        return box_tuple.to_bytes(t)

    def t_format(self, t: types.BoxTuple) -> types.TupleFormat:
        return box_tuple.format(t)

    def t_field(self, t: types.BoxTuple, idx: int) -> Optional[bytes]:
        return box_tuple.field(t, idx)

    def t_field_by_path(self, t: types.BoxTuple, path: str) -> Optional[bytes]:
        return box_tuple.field_by_path(t, path)

    def t_update(self, t: types.BoxTuple, expr: bytes) -> types.BoxTuple:
        return box_tuple.update(t, expr)

    def t_upsert(self, t: types.BoxTuple, expr: bytes) -> types.BoxTuple:
        return box_tuple.upsert(t, expr)

    def t_validate(self, t: types.BoxTuple, format: types.TupleFormat) -> None:
        box_tuple.validate(t, format)

    def t_compare(
        self, key_def_: types.KeyDef, left: types.BoxTuple, right: types.BoxTuple
    ) -> int:
        return box_tuple.compare(key_def_, left, right)

    def t_compare_with_key(
        self, tuple: types.BoxTuple, key: bytes, key_def_: types.KeyDef
    ) -> int:
        return box_tuple.compare_with_key(tuple, key, key_def_)

    def t_key_def_new(self, parts: List[types.KeyPart]) -> types.KeyDef:
        return key_def.new(parts)

    def t_tuple_format_default(self) -> types.TupleFormat:
        return tuple_format.default()

    def t_tuple_format_new(
        self, key_defs: List[types.KeyDef]
    ) -> types.TupleFormat:
        return tuple_format.new(key_defs)

    def t_iter_test_all(self, t: types.BoxTuple) -> bytes:
        result = []
        with box_tuple.Iterator.new(t) as iterator:
            while True:
                position = iterator.position()
                value = iterator.next()
                if value is None:
                    break
                result.append([position, json.loads(msgpack.to_json(value))])

            iterator.rewind()
            rewind_position = iterator.position()
            rewind_value = iterator.next()
            result.append(
                ["rewind", rewind_position, json.loads(msgpack.to_json(rewind_value))]
            )

            seek_value = iterator.seek(1)
            result.append(
                ["seek", iterator.position(), json.loads(msgpack.to_json(seek_value))]
            )

        return msgpack.from_json(json.dumps(result))
