from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some



def encode(data: bytes) -> bytes:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def decode(data: bytes) -> bytes:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def decode_from_raw_ptr(ptr: int) -> bytes:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

