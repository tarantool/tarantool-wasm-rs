from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some
from ..imports import tarantool_tarantool_types


def current() -> tarantool_tarantool_types.Session:
    raise NotImplementedError

def broadcast(key: str, value: str) -> None:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def iproto_send(session: tarantool_tarantool_types.Session, header: bytes, body: bytes) -> None:
    """
    Raises: `wit_world.types.Err(wit_world.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

