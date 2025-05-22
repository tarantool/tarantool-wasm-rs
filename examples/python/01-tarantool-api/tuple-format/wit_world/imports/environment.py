from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some



def get_environment() -> List[Tuple[str, str]]:
    raise NotImplementedError

def get_arguments() -> List[str]:
    raise NotImplementedError

def initial_cwd() -> Optional[str]:
    raise NotImplementedError

