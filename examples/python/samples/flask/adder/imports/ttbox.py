from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some
from ..imports import tarantool_tarantool_types


def schema_version() -> int:
    """
    Returns the current schema version of the database.
    
    The schema version increases whenever the schema changes, such as creating or dropping spaces or indexes.
    
    ### Returns
    - A 64-bit integer representing the current schema version.
    """
    raise NotImplementedError

def space_by_name(name: str) -> tarantool_tarantool_types.Space:
    """
    Retrieves a space object by its name.
    
    This function performs a lookup for a space by name and returns a reference used in subsequent operations.
    
    ### Parameters
    - `name`: The name of the space to retrieve.
    
    ### Returns
    - A space object if found, or an error otherwise.
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def index_by_name(space: tarantool_tarantool_types.Space, name: str) -> tarantool_tarantool_types.Index:
    """
    Retrieves an index object by name within a space.
    
    This function looks up the specified index in the given space and returns a reference to it.
    
    ### Parameters
    - `space`: The space to search within.
    - `name`: The name of the index to retrieve.
    
    ### Returns
    - An index object if found, or an error otherwise.
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def insert(space: tarantool_tarantool_types.Space, tup: bytes) -> int:
    """
    Inserts a new tuple into a space.
    
    The tuple must be provided as a MsgPack-encoded array: `[field1, field2, ...]`.
    
    ### Parameters
    - `space`: The target space.
    - `tup`: A MsgPack-encoded array representing the tuple.
    
    ### Returns
    - The inserted tuple.
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def update(index: tarantool_tarantool_types.Index, key: bytes, ops: bytes) -> int:
    """
    Updates an existing tuple by key in the specified index.
    
    - `key` must be a MsgPack array identifying the tuple, e.g. `[1]`
    - `ops` must be a MsgPack array of operations, e.g. `[[ "=", 2, 30 ]]`
    
    ### Parameters
    - `index`: The index to use for locating the tuple.
    - `key`: MsgPack-encoded key array.
    - `ops`: MsgPack-encoded array of update operations.
    
    ### Returns
    - The updated tuple.
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def replace(space: tarantool_tarantool_types.Space, tup: bytes) -> int:
    """
    Replaces an existing tuple or inserts a new one.
    
    ### Parameters
    - `space`: The space to operate on.
    - `tup`: A MsgPack-encoded array representing the new tuple.
    
    ### Returns
    - The replaced tuple.
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def upsert(index: tarantool_tarantool_types.Index, tup: bytes, ops: bytes) -> None:
    """
    Performs an UPSERT operation: update if exists, insert otherwise.
    
    ### Parameters
    - `index`: The index used for key lookup.
    - `tup`: A MsgPack-encoded tuple to insert.
    - `ops`: A MsgPack-encoded array of update operations.
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def delete(index: tarantool_tarantool_types.Index, key: bytes) -> int:
    """
    Deletes a tuple by key from the specified index.
    
    The key must be encoded as a MsgPack array, e.g. `[1]`.
    
    ### Parameters
    - `index`: The index used for key lookup.
    - `key`: MsgPack-encoded key array.
    
    ### Returns
    - The deleted tuple, if found.
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def truncate(space: tarantool_tarantool_types.Space) -> None:
    """
    Removes all tuples from the space.
    
    This operation is equivalent to SQL's `TRUNCATE TABLE`.
    
    ### Parameters
    - `space`: The target space to truncate.
    
    ### Returns
    - `null` on success, or an error.
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

