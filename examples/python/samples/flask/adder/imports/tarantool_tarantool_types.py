from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some


@dataclass
class BoxError:
    message: str
    type: str
    code: int
    payload: Optional[List[Tuple[str, str]]]
    file: Optional[str]
    line: Optional[int]

@dataclass
class Space:
    id: int

@dataclass
class Index:
    id: int
    space_id: int
    index_base: int

@dataclass
class LogContext:
    filename: str
    line: int

class LogLevel(Enum):
    S_FATAL = 0
    S_SYSERROR = 1
    S_ERROR = 2
    S_CRIT = 3
    S_WARN = 4
    S_INFO = 5
    S_VERBOSE = 6
    S_DEBUG = 7

@dataclass
class Session:
    id: int

class TxnIsolationLevel(Enum):
    TXN_ISOLATION_DEFAULT = 0
    TXN_ISOLATION_BEST_EFFORT = 1
    TXN_ISOLATION_READ_COMMITTED = 2
    TXN_ISOLATION_READ_CONFIRMED = 3
    TXN_ISOLATION_LINEARIZABLE = 4

class IteratorType(Enum):
    ITER_EQ = 0
    ITER_REQ = 1
    ITER_ALL = 2
    ITER_LT = 3
    ITER_LE = 4
    ITER_GE = 5
    ITER_GT = 6
    ITER_BITS_ALL_SET = 7
    ITER_BITS_ANY_SET = 8
    ITER_BITS_ALL_NOT_SET = 9
    ITER_OVERLAPS = 10
    ITER_NEIGHBOR = 11
    ITER_NP = 12
    ITER_PP = 13

class Iterator:
    
    @classmethod
    def new_iterator(cls, index: Index, iterator_type: IteratorType, key: bytes) -> Self:
        """
        Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
        """
        raise NotImplementedError
    def next(self) -> int:
        """
        Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
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


@dataclass
class Sequence:
    id: int

class KeyPartFlags(Flag):
    IS_NULLABLE = auto()
    EXCLUDE_NULL = auto()

@dataclass
class KeyPart:
    field_no: int
    field_type: str
    collation: Optional[str]
    path: Optional[str]
    flags: KeyPartFlags


