from wit_world import exports
from wit_world import R, V, E, F
from typing import Optional, Tuple


class Run(exports.Run):
    def run(self) -> None:
        pass

class Nums(exports.Nums):
    def t_bool(self, x: bool) -> bool:
        return x
    
    def t_s8(self, x: int) -> int:
        return x
    
    def t_s16(self, x: int) -> int:
        return x
    
    def t_s32(self, x: int) -> int:
        return x
    
    def t_s64(self, x: int) -> int:
        return x
    
    def t_u8(self, x: int) -> int:
        return x
    
    def t_u16(self, x: int) -> int:
        return x
    
    def t_u32(self, x: int) -> int:
        return x
    
    def t_u64(self, x: int) -> int:
        return x
    
    def t_f32(self, x: float) -> float:
        return x
    
    def t_f64(self, x: float) -> float:
        return x


class WitWorld:
    
    def t_char(self, x: str) -> str:
        return x
    
    def t_string(self, x: str) -> str:
        return x
    
    def t_list(self, x: str) -> str:
        return x

    def t_option(self, x: Optional[int]) -> Optional[int]:
        return x
    
    def t_result(self, x: bool) -> bool:
        """
        Raises: `wit_world.types.Err(wit_world.imports.str)`
        """
        if x:
            return x
        else:
            raise Exception(f"Error: {x}")
        
    
    def t_tuple(self, x: Tuple[bool, float, str]) -> Tuple[bool, str]:
        return (x[0], x[2])
    
    def t_record(self, x: R) -> R:
        return x
    
    def t_variant(self, x: V) -> V:
        return x

    def t_enum(self, x: E) -> E:
        return x
    
    def t_flags(self, x: F) -> F:
        return x
