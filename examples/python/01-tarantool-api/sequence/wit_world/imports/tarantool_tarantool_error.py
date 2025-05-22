from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some
from ..imports import tarantool_tarantool_types


def error_new(message: str, type: str, code: int, file: Optional[str], line: Optional[int]) -> tarantool_tarantool_types.BoxError:
    raise NotImplementedError

def set(err: tarantool_tarantool_types.BoxError) -> None:
    raise NotImplementedError

def last() -> Optional[tarantool_tarantool_types.BoxError]:
    raise NotImplementedError

def clear() -> None:
    raise NotImplementedError

def to_string(err: tarantool_tarantool_types.BoxError) -> str:
    raise NotImplementedError

