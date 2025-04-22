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
    Returns the number of tuples stored in the index.
    
    ### Parameters
    - `index`: The index to query.
    
    ### Returns
    - The number of tuples in the index, or a `box-error` on failure.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def bsize(index: tarantool_tarantool_types.Index) -> int:
    """
    Returns the memory size of the index in bytes.
    
    ### Parameters
    - `index`: The index to inspect.
    
    ### Returns
    - Size in bytes, or a `box-error`.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def random(index: tarantool_tarantool_types.Index, rnd: int) -> int:
    """
    Returns a random tuple from the index.
    
    ### Parameters
    - `index`: The index to read from.
    - `rnd`: A random seed value.
    
    ### Returns
    - A random tuple or a `box-error`.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def get(index: tarantool_tarantool_types.Index, key: bytes) -> int:
    """
    Gets a tuple by key from the index.
    
    The key must be a MsgPack-encoded JSON array like `[1]`.
    
    ### Parameters
    - `index`: The index to search.
    - `key`: MsgPack-encoded key (as JSON UTF-8 string).
    
    ### Returns
    - The found tuple or an error.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def min(index: tarantool_tarantool_types.Index, key: bytes) -> int:
    """
    Finds the minimal tuple matching the given key.
    
    ### Parameters
    - `index`: The index to search.
    - `key`: MsgPack-encoded key.
    
    ### Returns
    - The smallest matching tuple or an error.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def max(index: tarantool_tarantool_types.Index, key: bytes) -> int:
    """
    Finds the maximal tuple matching the given key.
    
    ### Parameters
    - `index`: The index to search.
    - `key`: MsgPack-encoded key.
    
    ### Returns
    - The largest matching tuple or an error.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def count(index: tarantool_tarantool_types.Index, iter_type: tarantool_tarantool_types.IteratorType, key: bytes) -> int:
    """
    Counts the number of tuples matching the given key and iterator type.
    
    ### Parameters
    - `index`: Index to query.
    - `iter-type`: One of `iterator-type` values.
    - `key`: MsgPack-encoded key.
    
    ### Returns
    - Count of matching tuples, or an error.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

