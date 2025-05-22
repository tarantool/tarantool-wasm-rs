from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some
from ..imports import tarantool_tarantool_types


def new(parts: List[tarantool_tarantool_types.KeyPart]) -> int:
    """
    Creates a new key definition from a list of parts.
    
    Each key part defines one field to be used in a key for indexing or validation.
    The `KeyPart` structure includes field number, type, optional collation, path, and flags.
    
    ### Parameters
    - `parts`: List of `key-part` objects.
    
    ### Returns
    - A new `key-def` object on success.
    
    ### Raises
    - `box-error
    
    **Python**
    ```
    parts = [
    KeyPart(field_no=0, field_type="unsigned", collation=None, path=None,
    flags=KeyPartFlags.IS_NULLABLE),
    KeyPart(field_no=1, field_type="string", collation="unicode", path=None,
    flags=KeyPartFlags.EXCLUDE_NULL)
    ]
    kd = key_def.new(parts)
    ```
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def dup(key_def: int) -> int:
    """
    Creates a deep copy of the given key definition.
    
    The returned `key-def` is independent of the original and must be deleted manually.
    
    ### Parameters
    - `key-def`: The key definition to duplicate.
    
    ### Returns
    - A duplicated `key-def`.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def merge(left: int, right: int) -> int:
    """
    Merges two key definitions into one.
    
    Fields from the right-hand side are appended if they are not duplicates of the left.
    The resulting merged definition can be used for composite index construction.
    
    ### Parameters
    - `left`: The base key definition.
    - `right`: The key definition to append fields from.
    
    ### Returns
    - A new merged `key-def`.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def delete(key_def: int) -> None:
    """
    Deletes a key definition, freeing any associated memory.
    
    You **must** call this for all key definitions you create with `new`, `dup`, or `merge`.
    
    ### Parameters
    - `key-def`: The key definition to delete.
    
    ### Raises
    - `box-error
    """
    raise NotImplementedError

def dump_parts(key_def: int) -> List[tarantool_tarantool_types.KeyPart]:
    """
    Returns a list of key parts from a given key definition.
    
    Useful for debugging and introspection — lets you inspect fields and their order.
    
    ### Parameters
    - `key-def`: The key definition to inspect.
    
    ### Returns
    - List of `key-part` objects.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def validate_key(key_def: int, key: bytes) -> Tuple[bool, int]:
    """
    Validates whether a given key conforms to the key definition.
    
    Checks the number of fields and their types, allowing partial matches
    (e.g. prefix of the full key).
    
    ### Parameters
    - `key-def`: Key definition to validate against.
    - `key`: MsgPack-encoded key (e.g., `[1, "abc"]`, encoded with `msgpack.encode(json.dumps(...))`)
    
    ### Returns
    - A tuple `(is_valid: bool, size: u32)`
    - `is_valid`: Whether the key is valid.
    - `size`: Size of decoded key in bytes (if valid), or 0 if invalid.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def validate_full_key(key_def: int, key: bytes) -> Tuple[bool, int]:
    """
    Validates whether a key **fully** matches the key definition.
    
    Differs from `validate-key` by requiring exact number of parts.
    
    ### Parameters
    - `key-def`: Key definition to validate against.
    - `key`: MsgPack-encoded key (e.g., `[1, "abc"]`)
    
    ### Returns
    - A tuple `(is_valid: bool, size: u32)` like in `validate-key`.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def part_count(key_def: int) -> int:
    """
    Returns the number of parts in the key definition.
    
    Each part corresponds to one field used in index comparison or validation.
    
    ### Parameters
    - `key-def`: The key definition.
    
    ### Returns
    - Number of parts as `u32`.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def extract_key(key_def: int, tuple: int) -> int:
    """
    Extracts a key from a given tuple based on the key definition.
    
    The result is a `box-tuple` containing only the fields specified by the key definition.
    
    ### Parameters
    - `key-def`: Key definition describing how to extract fields.
    - `tuple`: Source `box-tuple`.
    
    ### Returns
    - A new `box-tuple` with key fields only.
    """
    raise NotImplementedError

def validate_tuple(key_def: int, tuple: int) -> bool:
    """
    Validates whether the tuple contains all required fields
    and types as specified by the key definition.
    
    This is useful to check if a `box-tuple` is compatible with an index.
    
    ### Parameters
    - `key-def`: Key definition to validate against.
    - `tuple`: A `box-tuple` to validate.
    
    ### Returns
    - `true` if the tuple satisfies the key definition, otherwise `false`.
    """
    raise NotImplementedError

