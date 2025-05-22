from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some
from ..imports import tarantool_tarantool_types


def schema_version() -> int:
    raise NotImplementedError

def space_by_name(name: str) -> tarantool_tarantool_types.Space:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def index_by_name(space: tarantool_tarantool_types.Space, name: str) -> tarantool_tarantool_types.Index:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def insert(space: tarantool_tarantool_types.Space, tup: bytes) -> int:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def update(index: tarantool_tarantool_types.Index, key: bytes, ops: bytes) -> int:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def replace(space: tarantool_tarantool_types.Space, tup: bytes) -> int:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def upsert(index: tarantool_tarantool_types.Index, tup: bytes, ops: bytes) -> None:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def delete(index: tarantool_tarantool_types.Index, key: bytes) -> int:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def truncate(space: tarantool_tarantool_types.Space) -> None:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

