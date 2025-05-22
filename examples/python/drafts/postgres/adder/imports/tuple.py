from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some


class TupleIterator:
    
    @classmethod
    def new_tuple_iterator(cls, tuple: int) -> Self:
        """
        Creates a new iterator over the fields of a tuple.
        
        This allows sequential or random access to MsgPack fields within the tuple.
        
        ### Parameters
        - `tuple`: The tuple to iterate.
        
        ### Returns
        - A new `tuple-iterator` object, or a `box-error`.
        
        ### Raises
        - `box-error
        
        Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
        """
        raise NotImplementedError
    def position(self) -> int:
        """
        Returns the current iterator position (index of the next field).
        
        ### Returns
        - The next field index (zero-based).
        """
        raise NotImplementedError
    def rewind(self) -> None:
        """
        Rewinds the iterator to the beginning of the tuple.
        
        After this call, the next field will be the first.
        """
        raise NotImplementedError
    def seek(self, pos: int) -> int:
        """
        Seeks to a given field index in the tuple.
        
        The next call to `next()` will return this field.
        
        ### Parameters
        - `pos`: Zero-based field index to seek to.
        
        ### Returns
        - `tuple-field` pointer.
        
        ### Raises
        - `box-error
        
        Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
        """
        raise NotImplementedError
    def next(self) -> int:
        """
        Returns the next field in the tuple.
        
        Returns `null` if there are no more fields.
        
        ### Returns
        - `tuple-field` pointer.
        
        ### Raises
        - `box-error
        
        Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
        """
        raise NotImplementedError
    def __enter__(self) -> Self:
        """Returns self"""
        return self
                                
    def __exit__(self, exc_type: type[BaseException] | None, exc_value: BaseException | None, traceback: TracebackType | None) -> bool | None:
        """
        Release this resource.
        """
        raise NotImplementedError



def new(data: bytes) -> int:
    """
    Creates a new tuple using the specified format and raw MsgPack data.
    
    The data must be a MsgPack-encoded array (e.g., `[1, "abc", {"x": 42}]`).
    
    ### Parameters
    - `format`: A tuple format object.
    - `data`: MsgPack-encoded data (e.g., via `msgpack.encode(json.dumps(...))`)
    
    ### Returns
    - A new `box-tuple`, or `box-error`.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def ref(t: int) -> int:
    """
    Increases the reference count of the tuple.
    
    Call this if you want to store the tuple beyond the current function scope.
    
    ### Parameters
    - `t`: The tuple to retain.
    
    ### Returns
    - The new reference count as `s32`.
    """
    raise NotImplementedError

def unref(t: int) -> None:
    """
    Decreases the reference count of the tuple.
    
    If the count reaches zero, the tuple is freed.
    
    ### Parameters
    - `t`: The tuple to release.
    """
    raise NotImplementedError

def field_count(t: int) -> int:
    """
    Returns the number of fields in the tuple (i.e., array size).
    
    ### Parameters
    - `t`: The tuple.
    
    ### Returns
    - Field count as `u32`.
    """
    raise NotImplementedError

def bsize(t: int) -> int:
    """
    Returns the byte size of the internal MsgPack data.
    
    Useful for buffer allocations.
    
    ### Parameters
    - `t`: The tuple.
    
    ### Returns
    - Number of bytes used as `u64`.
    """
    raise NotImplementedError

def to_buf(t: int) -> bytes:
    """
    Serializes the tuple back to MsgPack byte array.
    
    This data can be passed to `decode` or reused elsewhere.
    
    ### Parameters
    - `t`: The tuple.
    
    ### Returns
    - A MsgPack-encoded byte array, or `box-error`.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def format(t: int) -> int:
    """
    Returns the format object associated with the tuple.
    
    ### Parameters
    - `t`: The tuple.
    
    ### Returns
    - The `tuple-format` used to construct the tuple.
    
    ### Raises
    - `box-error
    """
    raise NotImplementedError

def field(t: int, idx: int) -> int:
    """
    Returns a pointer to the field at the specified index (zero-based).
    
    The returned value is a pointer to raw MsgPack-encoded data and must be decoded.
    
    ### Parameters
    - `t`: The tuple.
    - `idx`: Field index (starting from 0).
    
    ### Returns
    - `tuple-field` (pointer), or `box-error`.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def field_by_path(t: int, path: str, index_base: int) -> int:
    """
    Returns a pointer to the field by JSON path expression.
    
    Index base:
    - 0 → C-style (zero-based)
    - 1 → Lua-style (one-based)
    
    ### Parameters
    - `t`: The tuple.
    - `path`: Path expression (e.g., `"[2].x"`).
    - `index-base`: Either 0 or 1.
    
    ### Returns
    - `tuple-field` (pointer).
    
    ### Raises
    - `box-error
    
    ### Examples
    
    **Python**
    ```
    field = tt_tuple.field_by_path(tup, "[2].x", 1)
    value = json.loads(msgpack.decode_from_raw_ptr(field).decode('utf-8'))
    ```
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def update(t: int, expr: bytes) -> int:
    """
    Updates fields in a tuple using the given update expression.
    
    The expression must be a MsgPack-encoded array of operations like:
    `[["=", 1, "new_value"]]`, where the first item is an operation, second is field index, and third is the value.
    
    ### Parameters
    - `t`: The tuple to update.
    - `expr`: MsgPack-encoded update expression.
    
    ### Returns
    - A new `box-tuple` with applied updates, or a `box-error`.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def upsert(t: int, expr: bytes) -> int:
    """
    Performs an upsert operation on the tuple using the provided expression.
    
    This operation applies updates if the field exists or appends otherwise.
    Common upsert usage includes incrementing fields: `[["+", 2, 1]]`.
    
    ### Parameters
    - `t`: The tuple.
    - `expr`: MsgPack-encoded upsert expression.
    
    ### Returns
    - A new `box-tuple` with result, or a `box-error`.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def validate(t: int, format: int) -> bool:
    """
    Validates a tuple against the given format.
    
    Checks field types and nullability constraints.
    
    ### Parameters
    - `t`: The tuple to validate.
    - `format`: Tuple format to validate against.
    
    ### Returns
    - `true` if valid, `false` otherwise.
    
    ### Raises
    - `box-error
    """
    raise NotImplementedError

def compare(key_def: int, left: int, right: int) -> int:
    """
    Compares two tuples using a given key definition.
    
    The comparison is lexicographical by key fields.
    
    ### Parameters
    - `key-def`: Definition of fields to compare.
    - `left`: First tuple.
    - `right`: Second tuple.
    
    ### Returns
    - `0` if equal
    - `<0` if `left < right`
    - `>0` if `left > right`
    """
    raise NotImplementedError

def compare_with_key(tuple: int, key: bytes, key_def: int) -> int:
    """
    Compares a tuple with a key using the given key definition.
    
    ### Parameters
    - `tuple`: Tuple to compare.
    - `key`: MsgPack-encoded key (e.g., `[1]`)
    - `key-def`: Key definition.
    
    ### Returns
    - `0` if key matches tuple
    - `<0` if tuple < key
    - `>0` if tuple > key
    """
    raise NotImplementedError

