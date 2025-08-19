from wit_world.imports import types, tuple_format, key_def
from typing import List


class WitWorld:

    def t_default(self) -> int:
        return tuple_format.default()
    
    def t_new(self, key_defs: List[types.KeyDef]) -> types.TupleFormat:
        return tuple_format.new(key_defs)

    def t_ref(self, tf: types.TupleFormat) -> None:
        tuple_format.ref(tf)

    def t_unref(self, tf: types.TupleFormat) -> None:
        tuple_format.unref(tf)

    def t_key_def_new(self, parts: List[types.KeyPart]) -> types.KeyDef:
       return key_def.new(parts)
