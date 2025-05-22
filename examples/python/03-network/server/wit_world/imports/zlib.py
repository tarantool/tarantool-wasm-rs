from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some



def compress(data: bytes, level: int) -> bytes:
    raise NotImplementedError

def decompress(data: bytes) -> bytes:
    raise NotImplementedError

def crc32(data: bytes) -> int:
    raise NotImplementedError

def adler32(data: bytes) -> int:
    raise NotImplementedError

