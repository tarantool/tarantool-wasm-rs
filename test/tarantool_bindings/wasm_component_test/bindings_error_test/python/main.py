from wit_world.imports import error, types
from typing import Optional

class WitWorld():

    def t_error_new(self, message: str, type: str, code: int, file: Optional[str], line: Optional[int]) -> types.BoxError:
        return error.error_new(message, type, code, file, line)
    
    def t_set(self, err: types.BoxError) -> None:
        error.set(err)

    def t_last(self) -> Optional[types.BoxError]:
        return error.last()

    def t_clear(self) -> None:
        error.clear()

    def t_to_string(self, err: types.BoxError) -> str:
        return error.to_string(err)
