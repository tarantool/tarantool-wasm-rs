from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some
from ..imports import tarantool_tarantool_types


def next(seq: tarantool_tarantool_types.Sequence) -> int:
    """
    Generates the next value of the specified sequence.
    
    This updates the state of the sequence.
    
    ### Parameters
    - `seq`: The target sequence object.
    
    ### Returns
    - The next value of the sequence as `s64`.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def current(seq: tarantool_tarantool_types.Sequence) -> int:
    """
    Returns the current value of the specified sequence.
    
    Unlike `next()`, this does **not** increment the sequence.
    Returns an error if the sequence was never accessed via `next()` or `set()`.
    
    ### Parameters
    - `seq`: The target sequence object.
    
    ### Returns
    - The current value of the sequence as `s64`.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def set(seq: tarantool_tarantool_types.Sequence, value: int) -> None:
    """
    Sets the sequence to a specific value.
    
    This function is useful when you want to jump to a specific number in the sequence.
    
    ### Parameters
    - `seq`: The target sequence object.
    - `value`: The new value to assign.
    
    ### Raises
    - `box-error``
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def reset(seq: tarantool_tarantool_types.Sequence) -> None:
    """
    Resets the sequence to its initial state.
    
    This call clears the current sequence value, effectively "forgetting" the state.
    After a reset, calling `current()` will return an error until `next()` or `set()` is called again.
    
    Internally, if no transaction is active, this function starts and commits one.
    
    ### Parameters
    - `seq`: The target sequence object.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

