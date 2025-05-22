from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some
from ..imports import tarantool_tarantool_types


def say(level: tarantool_tarantool_types.LogLevel, msg: str, ctx: Optional[tarantool_tarantool_types.LogContext]) -> None:
    raise NotImplementedError

def say_error(msg: str, ctx: Optional[tarantool_tarantool_types.LogContext]) -> None:
    raise NotImplementedError

def say_crit(msg: str, ctx: Optional[tarantool_tarantool_types.LogContext]) -> None:
    raise NotImplementedError

def say_warn(msg: str, ctx: Optional[tarantool_tarantool_types.LogContext]) -> None:
    raise NotImplementedError

def say_info(msg: str, ctx: Optional[tarantool_tarantool_types.LogContext]) -> None:
    raise NotImplementedError

def say_verbose(msg: str, ctx: Optional[tarantool_tarantool_types.LogContext]) -> None:
    raise NotImplementedError

def say_debug(msg: str, ctx: Optional[tarantool_tarantool_types.LogContext]) -> None:
    raise NotImplementedError

def say_syserror(msg: str, ctx: Optional[tarantool_tarantool_types.LogContext]) -> None:
    raise NotImplementedError

