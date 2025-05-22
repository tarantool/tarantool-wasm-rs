from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some


class TupleIterator:
    
    @classmethod
    def new_tuple_iterator(cls, tuple: int) -> Self:
        """
        Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
        """
        raise NotImplementedError
    def position(self) -> int:
        raise NotImplementedError
    def rewind(self) -> None:
        raise NotImplementedError
    def seek(self, pos: int) -> int:
        """
        Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
        """
        raise NotImplementedError
    def next(self) -> int:
        """
        Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
        """
        raise NotImplementedError
    def __enter__(self) -> Self:
        """Returns self"""
        return self
                                
    def __exit__(self, exc_type: type[BaseException] | None, exc_value: BaseException | None, traceback: TracebackType | None) -> bool | None:
        """
        Release this resource.
        """
        raise NotImplementedError



def new(data: bytes) -> int:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def ref(t: int) -> int:
    raise NotImplementedError

def unref(t: int) -> None:
    raise NotImplementedError

def field_count(t: int) -> int:
    raise NotImplementedError

def bsize(t: int) -> int:
    raise NotImplementedError

def to_buf(t: int) -> bytes:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def format(t: int) -> int:
    raise NotImplementedError

def field(t: int, idx: int) -> int:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def field_by_path(t: int, path: str, index_base: int) -> int:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def update(t: int, expr: bytes) -> int:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def upsert(t: int, expr: bytes) -> int:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def validate(t: int, format: int) -> bool:
    raise NotImplementedError

def compare(key_def: int, left: int, right: int) -> int:
    raise NotImplementedError

def compare_with_key(tuple: int, key: bytes, key_def: int) -> int:
    raise NotImplementedError

