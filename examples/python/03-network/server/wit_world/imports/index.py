from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some
from ..imports import tarantool_tarantool_types


def len(index: tarantool_tarantool_types.Index) -> int:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def bsize(index: tarantool_tarantool_types.Index) -> int:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def random(index: tarantool_tarantool_types.Index, rnd: int) -> int:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def get(index: tarantool_tarantool_types.Index, key: bytes) -> int:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def min(index: tarantool_tarantool_types.Index, key: bytes) -> int:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def max(index: tarantool_tarantool_types.Index, key: bytes) -> int:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def count(index: tarantool_tarantool_types.Index, iter_type: tarantool_tarantool_types.IteratorType, key: bytes) -> int:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

