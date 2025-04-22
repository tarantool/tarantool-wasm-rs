from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some
from ..imports import tarantool_tarantool_types


def say(level: tarantool_tarantool_types.LogLevel, msg: str, ctx: Optional[tarantool_tarantool_types.LogContext]) -> None:
    """
    Logs a message with the given log level.
    
    This is the generic logging function. If you know the level at compile time,
    consider using a level-specific function like `say-info` or `say-error`.
    
    ### Parameters
    - `level`: A value from `log-level`, such as `s-info` or `s-error`.
    - `msg`: The message string to log.
    
    ### Examples
    
    **Python**
    ```
    say.say(LogLevel.S_WARN, "This is a warning message")
    ```
    
    **JavaScript**
    ```
    say('s-warn', "This is a warning message");
    ```
    """
    raise NotImplementedError

def say_error(msg: str, ctx: Optional[tarantool_tarantool_types.LogContext]) -> None:
    """
    Logs an error message.
    
    ### Parameters
    - `msg`: The error message to log.
    
    ### Examples
    
    **Python**
    ```
    say.say_error("Something went wrong")
    ```
    
    **JavaScript**
    ```
    sayError("Something went wrong");
    ```
    """
    raise NotImplementedError

def say_crit(msg: str, ctx: Optional[tarantool_tarantool_types.LogContext]) -> None:
    """
    Logs a critical error message.
    
    ### Parameters
    - `msg`: The message to log.
    
    ### Examples
    
    **Python**
    ```
    say.say_crit("Unrecoverable condition encountered")
    ```
    
    **JavaScript**
    ```
    sayCrit("Unrecoverable condition encountered");
    ```
    """
    raise NotImplementedError

def say_warn(msg: str, ctx: Optional[tarantool_tarantool_types.LogContext]) -> None:
    """
    Logs a warning message.
    
    ### Parameters
    - `msg`: The message to log.
    
    ### Examples
    
    **Python**
    ```
    say.say_warn("This is just a warning")
    ```
    
    **JavaScript**
    ```
    sayWarn("This is just a warning");
    ```
    """
    raise NotImplementedError

def say_info(msg: str, ctx: Optional[tarantool_tarantool_types.LogContext]) -> None:
    """
    Logs an informational message.
    
    ### Parameters
    - `msg`: The message to log.
    
    ### Examples
    
    **Python**
    ```
    say.say_info("Processing started")
    ```
    
    **JavaScript**
    ```
    sayInfo("Processing started");
    ```
    """
    raise NotImplementedError

def say_verbose(msg: str, ctx: Optional[tarantool_tarantool_types.LogContext]) -> None:
    """
    Logs a verbose message for detailed output.
    
    ### Parameters
    - `msg`: The verbose message.
    
    ### Examples
    
    **Python**
    ```
    say.say_verbose("Detailed output")
    ```
    
    **JavaScript**
    ```
    sayVerbose("Detailed output");
    ```
    """
    raise NotImplementedError

def say_debug(msg: str, ctx: Optional[tarantool_tarantool_types.LogContext]) -> None:
    """
    Logs a debug message.
    
    ### Parameters
    - `msg`: The debug message.
    
    ### Examples
    
    **Python**
    ```
    say.say_debug("Debug info")
    ```
    
    **JavaScript**
    ```
    sayDebug("Debug info");
    ```
    """
    raise NotImplementedError

def say_syserror(msg: str, ctx: Optional[tarantool_tarantool_types.LogContext]) -> None:
    """
    Logs a system-level error message (e.g. related to `errno`).
    
    Typically used for OS-level failures.
    
    ### Parameters
    - `msg`: The system error description.
    
    ### Examples
    
    **Python**
    ```
    say.say_syserror("Failed to open file")
    ```
    
    **JavaScript**
    ```
    saySyserror("Failed to open file");
    ```
    """
    raise NotImplementedError

