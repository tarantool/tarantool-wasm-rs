from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some
from ..imports import tarantool_tarantool_types


def current() -> tarantool_tarantool_types.Session:
    """
    Returns the current session.
    
    The session object uniquely identifies the connection or context
    (e.g. a client connection) from which the current code is executed.
    
    ### Returns
    - A `session` object representing the current session.
    """
    raise NotImplementedError

def broadcast(key: str, value: str) -> None:
    """
    Broadcasts a message to all watchers subscribed to the given key.
    
    The `value` is interpreted as a string representation of a Lua object.
    Internally maps to `box.broadcast(key, value)` from Lua.
    
    ### Parameters
    - `key`: A string event name to broadcast on.
    - `value`: A stringified Lua object, e.g. `"{a = 1}"`.
    
    ### Returns
    - `ok` on success, or a `box-error` on failure.
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def iproto_send(session: tarantool_tarantool_types.Session, header: bytes, body: bytes) -> None:
    """
    Sends a raw IPROTO packet to a specific session.
    
    Both the header and body must be valid MsgPack-encoded buffers.
    
    Internally maps to `box_iproto_send(...)` from the C API.
    
    ### Parameters
    - `session`: Target session to send to.
    - `header`: MsgPack-encoded map containing the request header.
    - `body`: MsgPack-encoded body (optional but usually required).
    
    ### Returns
    - `ok` on success
    
    ### Raises
    - `box-error`
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

