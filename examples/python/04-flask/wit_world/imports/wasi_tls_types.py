from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some
from ..imports import streams
from ..imports import wasi_io_error
from ..imports import poll

class ClientConnection:
    
    def close_output(self) -> None:
        raise NotImplementedError
    def __enter__(self) -> Self:
        """Returns self"""
        return self
                                
    def __exit__(self, exc_type: type[BaseException] | None, exc_value: BaseException | None, traceback: TracebackType | None) -> bool | None:
        """
        Release this resource.
        """
        raise NotImplementedError


class FutureClientStreams:
    
    def subscribe(self) -> poll.Pollable:
        raise NotImplementedError
    def get(self) -> Optional[Result[Result[Tuple[ClientConnection, streams.InputStream, streams.OutputStream], wasi_io_error.Error], None]]:
        raise NotImplementedError
    def __enter__(self) -> Self:
        """Returns self"""
        return self
                                
    def __exit__(self, exc_type: type[BaseException] | None, exc_value: BaseException | None, traceback: TracebackType | None) -> bool | None:
        """
        Release this resource.
        """
        raise NotImplementedError


class ClientHandshake:
    
    def __init__(self, server_name: str, input: streams.InputStream, output: streams.OutputStream) -> None:
        raise NotImplementedError

    @classmethod
    def finish(cls, this: Self) -> FutureClientStreams:
        raise NotImplementedError
    def __enter__(self) -> Self:
        """Returns self"""
        return self
                                
    def __exit__(self, exc_type: type[BaseException] | None, exc_value: BaseException | None, traceback: TracebackType | None) -> bool | None:
        """
        Release this resource.
        """
        raise NotImplementedError



