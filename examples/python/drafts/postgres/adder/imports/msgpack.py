from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some



def encode(data: bytes) -> bytes:
    """
    Converts a JSON string into MessagePack format.
    
    Accepts a UTF-8 encoded JSON string as a byte array and returns the equivalent
    structure encoded in MessagePack format.
    
    This is useful for sending structured data from higher-level languages
    to Tarantool via MessagePack, using familiar JSON as input.
    
    ### Parameters
    - `data`: A byte array containing a valid UTF-8 encoded JSON string.
    
    ### Returns
    - A byte array containing the structure in MessagePack format.
    
    ### Raises
    - BoxError
    
    ### Examples
    
    **Python**
    ```
    obj = [1, "Alice", 25]
    json_bytes = json.dumps(obj).encode('utf-8')
    encoded_msgpack = msgpack.encode(json_bytes)
    ```
    
    **JavaScript**
    ```
    const obj = [1, "Alice", 25];
    const jsonBytes = new TextEncoder().encode(JSON.stringify(obj));
    const encoded = await encode(jsonBytes);
    ```
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def decode(data: bytes) -> bytes:
    """
    Decodes a MessagePack-encoded byte array into a UTF-8 JSON string.
    
    This function accepts a byte array containing data in MessagePack format,
    deserializes it into an intermediate structure, and serializes it into a JSON string,
    encoded as UTF-8 bytes.
    
    ### Parameters
    - `data`: A byte array containing data encoded in MessagePack format.
    
    ### Returns
    - On success, returns a byte array representing a UTF-8 encoded JSON string.
    
    ### Raises
    - BoxError
    
    ### Examples
    
    **Python**
    ```
    decoded_json_bytes = msgpack.decode(msgpack_bytes)
    json_string = decoded_json_bytes.decode('utf-8')
    obj = json.loads(json_string)
    ```
    
    **JavaScript**
    ```
    const jsonBytes = await decode(msgpackBytes);
    const jsonString = new TextDecoder().decode(jsonBytes);
    const obj = JSON.parse(jsonString);
    ```
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

def decode_from_raw_ptr(ptr: int) -> bytes:
    """
    Decodes a MessagePack-encoded value from a raw memory pointer into a UTF-8 JSON string.
    
    This function reads and decodes MessagePack data starting at the given memory address,
    which is typically obtained from a low-level system API or runtime interface.
    The resulting value is converted to a JSON string and returned as a UTF-8 encoded byte array.
    
    ### Parameters
    - `ptr`: A raw memory address (`u64`) pointing to the beginning of a valid MsgPack-encoded value.
    
    ### Returns
    - On success, returns a byte array containing a UTF-8 encoded JSON string.
    
    ### Raises
    - BoxError
    
    ### Safety
    This function performs an unsafe read from the specified memory location. The pointer must
    refer to a valid region of memory containing a complete MessagePack value of known or bounded size.
    
    ### Examples
    
    **Python**
    ```
    decoded_json_bytes = msgpack.decode_from_raw_ptr(ptr)
    json_str = decoded_json_bytes.decode('utf-8')
    obj = json.loads(json_str)
    ```
    
    **JavaScript**
    ```
    const jsonBytes = await decodeFromRawPtr(ptr);
    const jsonString = new TextDecoder().decode(jsonBytes);
    const obj = JSON.parse(jsonString);
    ```
    
    Raises: `adder.types.Err(adder.imports.tarantool_tarantool_types.BoxError)`
    """
    raise NotImplementedError

