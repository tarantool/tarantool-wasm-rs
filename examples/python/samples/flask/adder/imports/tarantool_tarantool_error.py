from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some
from ..imports import tarantool_tarantool_types


def error_new(message: str, type: str, code: int, file: Optional[str], line: Optional[int]) -> tarantool_tarantool_types.BoxError:
    """
    Creates a new error object with the given message, type, and error code.
    
    ### Parameters
    - `message`: The error message.
    - `type`: The error type name.
    - `code`: The numeric error code.
    
    ### Returns
    - A new `box-error` object.
    """
    raise NotImplementedError

def set(err: tarantool_tarantool_types.BoxError) -> None:
    """
    Sets the given error as the current "last error".
    
    After calling this, `error.last()` will return the same error until cleared or overwritten.
    
    ### Parameters
    - `err`: The error to set.
    """
    raise NotImplementedError

def last() -> Optional[tarantool_tarantool_types.BoxError]:
    """
    Returns the last error set by the system or user code.
    
    Similar to `errno` in C. Returns `none` if no error is currently set.
    
    ### Returns
    - `box-error` or `none` if no error is currently set.
    """
    raise NotImplementedError

def clear() -> None:
    """
    Clears the last error.
    
    This is typically used to reset the error state before performing new operations.
    """
    raise NotImplementedError

def to_string(err: tarantool_tarantool_types.BoxError) -> str:
    """
    Converts a `box-error` object to a string representation.
    
    ### Parameters
    - `err`: The error to stringify.
    
    ### Returns
    - A human-readable string describing the error.
    """
    raise NotImplementedError

