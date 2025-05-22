from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some
from ..imports import wasi_http_types

class IncomingHandler(Protocol):

    @abstractmethod
    def handle(self, request: wasi_http_types.IncomingRequest, response_out: wasi_http_types.ResponseOutparam) -> None:
        raise NotImplementedError


class Run(Protocol):

    @abstractmethod
    def run(self) -> None:
        """
        Raises: `wit_world.types.Err(None)`
        """
        raise NotImplementedError


