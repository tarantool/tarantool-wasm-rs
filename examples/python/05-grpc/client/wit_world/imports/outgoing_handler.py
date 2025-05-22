from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some
from ..imports import wasi_http_types


def handle(request: wasi_http_types.OutgoingRequest, options: Optional[wasi_http_types.RequestOptions]) -> wasi_http_types.FutureIncomingResponse:
    """
    Raises: `wit_world.types.Err(wit_world.imports.wasi_http_types.ErrorCode)`
    """
    raise NotImplementedError

