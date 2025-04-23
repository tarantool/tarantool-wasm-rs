from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some
from ..imports import tarantool_tarantool_types


def txn_id() -> int:
    """
    Returns the current transaction ID.
    
    A transaction ID is a non-persistent unique identifier of the current transaction.
    If no transaction is active, the return value is `-1`.
    
    ### Returns
    - A signed 64-bit integer representing the transaction ID, or `-1` if no transaction.
    """
    raise NotImplementedError

def txn_isolation() -> tarantool_tarantool_types.TxnIsolationLevel:
    """
    Returns the isolation level of the current transaction.
    
    Returns `-1` if there is no active transaction.
    
    ### Returns
    - A `txn-isolation-level` enum value indicating the current isolation level.
    """
    raise NotImplementedError

def txn() -> bool:
    """
    Checks whether a transaction is currently active.
    
    ### Returns
    - `true` if a transaction is active; `false` otherwise.
    """
    raise NotImplementedError

def begin() -> None:
    """
    Begins a new transaction.
    
    Must be called before any data manipulation (INSERT, UPDATE, etc.).
    Fails if a transaction is already active in the fiber.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def commit() -> None:
    """
    Commits the current transaction.
    
    Finalizes and applies all changes. If a disk error occurs, it returns an error.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def rollback() -> None:
    """
    Rolls back the current transaction.
    
    Discards all changes made within the current transaction.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def txn_set_isolation(isolation: tarantool_tarantool_types.TxnIsolationLevel) -> None:
    """
    Sets the isolation level for the current transaction.
    
    Must be called before the first DML operation in the transaction.
    
    ### Parameters
    - `isolation`: A `txn-isolation-level` enum value, such as `read-committed`.
    
    ### Raises
    - `box-error
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def txn_make_sync() -> None:
    """
    Marks the current transaction as synchronous.
    
    All changes will be synchronized to disk before commit completes.
    """
    raise NotImplementedError

