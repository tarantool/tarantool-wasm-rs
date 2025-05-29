from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some
from ..imports import tarantool_tarantool_types


def txn_id() -> int:
    raise NotImplementedError

def txn_isolation() -> tarantool_tarantool_types.TxnIsolationLevel:
    raise NotImplementedError

def txn() -> bool:
    raise NotImplementedError

def begin() -> None:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def commit() -> None:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def rollback() -> None:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def txn_set_isolation(isolation: tarantool_tarantool_types.TxnIsolationLevel) -> None:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def txn_make_sync() -> None:
    raise NotImplementedError

