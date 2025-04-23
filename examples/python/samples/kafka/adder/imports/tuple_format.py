from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some



def default() -> int:
    """
    Returns the default tuple format.
    
    This format is used to create tuples not bound to any specific space.
    
    ### Returns
    - A `tuple-format` object.
    """
    raise NotImplementedError

def new(key_defs: List[int]) -> int:
    """
    Creates a new tuple format based on a list of key definitions.
    
    Key definitions define constraints and structure (field types, paths, etc.).
    
    ### Parameters
    - `key-defs`: A list of key definitions that describe indexable parts.
    
    ### Returns
    - A `tuple-format` object.
    
    ### Raises
    - `box-error
    
    ### Examples
    
    **Python**
    ```
    parts = [
    KeyPart(field_no=0, field_type="unsigned", collation=None, path=None,
    flags=KeyPartFlags.IS_NULLABLE),
    KeyPart(field_no=1, field_type="string", collation=None, path=None,
    flags=KeyPartFlags.EXCLUDE_NULL)
    ]
    kd = key_def.new(parts)
    tf = tuple_format.new([kd])
    ```
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def ref(tf: int) -> None:
    """
    Increments the reference counter of the tuple format.
    
    You should call this if you plan to store the format and use it across calls or threads.
    
    ### Parameters
    - `tf`: Tuple format to retain.
    """
    raise NotImplementedError

def unref(tf: int) -> None:
    """
    Decrements the reference counter of the tuple format.
    
    Must be called to release the format if it was previously retained via `ref`.
    
    ### Parameters
    - `tf`: Tuple format to release.
    """
    raise NotImplementedError

