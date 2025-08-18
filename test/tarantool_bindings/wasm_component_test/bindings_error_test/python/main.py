from wit_world.imports import error, types
from typing import Optional

class WitWorld():

    def t_new(self, code: types.ErrorCode, message: str) -> types.BoxError:
        return error.new(code, message)

    def t_new_with_location(self, code: types.ErrorCode, message: str, file: Optional[str], line: Optional[int]) -> types.BoxError:
        return error.new_with_location(code, message, file, line)
    
    def t_set(self, err: types.BoxError) -> None:
        error.set(err)

    def t_last(self) -> Optional[types.BoxError]:
        return error.last()

    def t_clear(self) -> None:
        error.clear()

    def t_to_string(self, err: types.BoxError) -> str:
        return error.to_string(err)
