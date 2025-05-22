from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some



def default() -> int:
    raise NotImplementedError

def new(key_defs: List[int]) -> int:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def ref(tf: int) -> None:
    raise NotImplementedError

def unref(tf: int) -> None:
    raise NotImplementedError

