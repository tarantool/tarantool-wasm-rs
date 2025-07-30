from wit_world.imports import types, tuple_format, key_def
from typing import List


class WitWorld:

    def t_default(self) -> int:
        return tuple_format.default()
    
    def t_new(self, key_defs: List[int]) -> int:
        return tuple_format.new(key_defs)

    def t_ref(self, tf: int) -> None:
        tuple_format.ref(tf)

    def t_unref(self, tf: int) -> None:
        tuple_format.unref(tf)

    def t_key_def_new(self, parts: List[types.KeyPart]) -> int:
       return key_def.new(parts)
