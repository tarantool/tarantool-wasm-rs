from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from types import TracebackType
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from ..types import Result, Ok, Err, Some
from ..imports import poll
from ..imports import streams
from ..imports import wasi_io_error


@dataclass
class Method_Get:
    pass


@dataclass
class Method_Head:
    pass


@dataclass
class Method_Post:
    pass


@dataclass
class Method_Put:
    pass


@dataclass
class Method_Delete:
    pass


@dataclass
class Method_Connect:
    pass


@dataclass
class Method_Options:
    pass


@dataclass
class Method_Trace:
    pass


@dataclass
class Method_Patch:
    pass


@dataclass
class Method_Other:
    value: str


Method = Union[Method_Get, Method_Head, Method_Post, Method_Put, Method_Delete, Method_Connect, Method_Options, Method_Trace, Method_Patch, Method_Other]



@dataclass
class Scheme_Http:
    pass


@dataclass
class Scheme_Https:
    pass


@dataclass
class Scheme_Other:
    value: str


Scheme = Union[Scheme_Http, Scheme_Https, Scheme_Other]


@dataclass
class DnsErrorPayload:
    rcode: Optional[str]
    info_code: Optional[int]

@dataclass
class TlsAlertReceivedPayload:
    alert_id: Optional[int]
    alert_message: Optional[str]

@dataclass
class FieldSizePayload:
    field_name: Optional[str]
    field_size: Optional[int]


@dataclass
class ErrorCode_DnsTimeout:
    pass


@dataclass
class ErrorCode_DnsError:
    value: DnsErrorPayload


@dataclass
class ErrorCode_DestinationNotFound:
    pass


@dataclass
class ErrorCode_DestinationUnavailable:
    pass


@dataclass
class ErrorCode_DestinationIpProhibited:
    pass


@dataclass
class ErrorCode_DestinationIpUnroutable:
    pass


@dataclass
class ErrorCode_ConnectionRefused:
    pass


@dataclass
class ErrorCode_ConnectionTerminated:
    pass


@dataclass
class ErrorCode_ConnectionTimeout:
    pass


@dataclass
class ErrorCode_ConnectionReadTimeout:
    pass


@dataclass
class ErrorCode_ConnectionWriteTimeout:
    pass


@dataclass
class ErrorCode_ConnectionLimitReached:
    pass


@dataclass
class ErrorCode_TlsProtocolError:
    pass


@dataclass
class ErrorCode_TlsCertificateError:
    pass


@dataclass
class ErrorCode_TlsAlertReceived:
    value: TlsAlertReceivedPayload


@dataclass
class ErrorCode_HttpRequestDenied:
    pass


@dataclass
class ErrorCode_HttpRequestLengthRequired:
    pass


@dataclass
class ErrorCode_HttpRequestBodySize:
    value: Optional[int]


@dataclass
class ErrorCode_HttpRequestMethodInvalid:
    pass


@dataclass
class ErrorCode_HttpRequestUriInvalid:
    pass


@dataclass
class ErrorCode_HttpRequestUriTooLong:
    pass


@dataclass
class ErrorCode_HttpRequestHeaderSectionSize:
    value: Optional[int]


@dataclass
class ErrorCode_HttpRequestHeaderSize:
    value: Optional[FieldSizePayload]


@dataclass
class ErrorCode_HttpRequestTrailerSectionSize:
    value: Optional[int]


@dataclass
class ErrorCode_HttpRequestTrailerSize:
    value: FieldSizePayload


@dataclass
class ErrorCode_HttpResponseIncomplete:
    pass


@dataclass
class ErrorCode_HttpResponseHeaderSectionSize:
    value: Optional[int]


@dataclass
class ErrorCode_HttpResponseHeaderSize:
    value: FieldSizePayload


@dataclass
class ErrorCode_HttpResponseBodySize:
    value: Optional[int]


@dataclass
class ErrorCode_HttpResponseTrailerSectionSize:
    value: Optional[int]


@dataclass
class ErrorCode_HttpResponseTrailerSize:
    value: FieldSizePayload


@dataclass
class ErrorCode_HttpResponseTransferCoding:
    value: Optional[str]


@dataclass
class ErrorCode_HttpResponseContentCoding:
    value: Optional[str]


@dataclass
class ErrorCode_HttpResponseTimeout:
    pass


@dataclass
class ErrorCode_HttpUpgradeFailed:
    pass


@dataclass
class ErrorCode_HttpProtocolError:
    pass


@dataclass
class ErrorCode_LoopDetected:
    pass


@dataclass
class ErrorCode_ConfigurationError:
    pass


@dataclass
class ErrorCode_InternalError:
    value: Optional[str]


ErrorCode = Union[ErrorCode_DnsTimeout, ErrorCode_DnsError, ErrorCode_DestinationNotFound, ErrorCode_DestinationUnavailable, ErrorCode_DestinationIpProhibited, ErrorCode_DestinationIpUnroutable, ErrorCode_ConnectionRefused, ErrorCode_ConnectionTerminated, ErrorCode_ConnectionTimeout, ErrorCode_ConnectionReadTimeout, ErrorCode_ConnectionWriteTimeout, ErrorCode_ConnectionLimitReached, ErrorCode_TlsProtocolError, ErrorCode_TlsCertificateError, ErrorCode_TlsAlertReceived, ErrorCode_HttpRequestDenied, ErrorCode_HttpRequestLengthRequired, ErrorCode_HttpRequestBodySize, ErrorCode_HttpRequestMethodInvalid, ErrorCode_HttpRequestUriInvalid, ErrorCode_HttpRequestUriTooLong, ErrorCode_HttpRequestHeaderSectionSize, ErrorCode_HttpRequestHeaderSize, ErrorCode_HttpRequestTrailerSectionSize, ErrorCode_HttpRequestTrailerSize, ErrorCode_HttpResponseIncomplete, ErrorCode_HttpResponseHeaderSectionSize, ErrorCode_HttpResponseHeaderSize, ErrorCode_HttpResponseBodySize, ErrorCode_HttpResponseTrailerSectionSize, ErrorCode_HttpResponseTrailerSize, ErrorCode_HttpResponseTransferCoding, ErrorCode_HttpResponseContentCoding, ErrorCode_HttpResponseTimeout, ErrorCode_HttpUpgradeFailed, ErrorCode_HttpProtocolError, ErrorCode_LoopDetected, ErrorCode_ConfigurationError, ErrorCode_InternalError]



@dataclass
class HeaderError_InvalidSyntax:
    pass


@dataclass
class HeaderError_Forbidden:
    pass


@dataclass
class HeaderError_Immutable:
    pass


HeaderError = Union[HeaderError_InvalidSyntax, HeaderError_Forbidden, HeaderError_Immutable]


class Fields:
    
    def __init__(self) -> None:
        raise NotImplementedError

    @classmethod
    def from_list(cls, entries: List[Tuple[str, bytes]]) -> Self:
        """
        Raises: `wit_world.types.Err(wit_world.imports.wasi_http_types.HeaderError)`
        """
        raise NotImplementedError
    def get(self, name: str) -> List[bytes]:
        raise NotImplementedError
    def has(self, name: str) -> bool:
        raise NotImplementedError
    def set(self, name: str, value: List[bytes]) -> None:
        """
        Raises: `wit_world.types.Err(wit_world.imports.wasi_http_types.HeaderError)`
        """
        raise NotImplementedError
    def delete(self, name: str) -> None:
        """
        Raises: `wit_world.types.Err(wit_world.imports.wasi_http_types.HeaderError)`
        """
        raise NotImplementedError
    def append(self, name: str, value: bytes) -> None:
        """
        Raises: `wit_world.types.Err(wit_world.imports.wasi_http_types.HeaderError)`
        """
        raise NotImplementedError
    def entries(self) -> List[Tuple[str, bytes]]:
        raise NotImplementedError
    def clone(self) -> Self:
        raise NotImplementedError
    def __enter__(self) -> Self:
        """Returns self"""
        return self
                                
    def __exit__(self, exc_type: type[BaseException] | None, exc_value: BaseException | None, traceback: TracebackType | None) -> bool | None:
        """
        Release this resource.
        """
        raise NotImplementedError


class FutureTrailers:
    
    def subscribe(self) -> poll.Pollable:
        raise NotImplementedError
    def get(self) -> Optional[Result[Result[Optional[Fields], ErrorCode], None]]:
        raise NotImplementedError
    def __enter__(self) -> Self:
        """Returns self"""
        return self
                                
    def __exit__(self, exc_type: type[BaseException] | None, exc_value: BaseException | None, traceback: TracebackType | None) -> bool | None:
        """
        Release this resource.
        """
        raise NotImplementedError


class IncomingBody:
    
    def stream(self) -> streams.InputStream:
        """
        Raises: `wit_world.types.Err(None)`
        """
        raise NotImplementedError
    @classmethod
    def finish(cls, this: Self) -> FutureTrailers:
        raise NotImplementedError
    def __enter__(self) -> Self:
        """Returns self"""
        return self
                                
    def __exit__(self, exc_type: type[BaseException] | None, exc_value: BaseException | None, traceback: TracebackType | None) -> bool | None:
        """
        Release this resource.
        """
        raise NotImplementedError


class IncomingRequest:
    
    def method(self) -> Method:
        raise NotImplementedError
    def path_with_query(self) -> Optional[str]:
        raise NotImplementedError
    def scheme(self) -> Optional[Scheme]:
        raise NotImplementedError
    def authority(self) -> Optional[str]:
        raise NotImplementedError
    def headers(self) -> Fields:
        raise NotImplementedError
    def consume(self) -> IncomingBody:
        """
        Raises: `wit_world.types.Err(None)`
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


class OutgoingBody:
    
    def write(self) -> streams.OutputStream:
        """
        Raises: `wit_world.types.Err(None)`
        """
        raise NotImplementedError
    @classmethod
    def finish(cls, this: Self, trailers: Optional[Fields]) -> None:
        """
        Raises: `wit_world.types.Err(wit_world.imports.wasi_http_types.ErrorCode)`
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


class OutgoingRequest:
    
    def __init__(self, headers: Fields) -> None:
        raise NotImplementedError

    def body(self) -> OutgoingBody:
        """
        Raises: `wit_world.types.Err(None)`
        """
        raise NotImplementedError
    def method(self) -> Method:
        raise NotImplementedError
    def set_method(self, method: Method) -> None:
        """
        Raises: `wit_world.types.Err(None)`
        """
        raise NotImplementedError
    def path_with_query(self) -> Optional[str]:
        raise NotImplementedError
    def set_path_with_query(self, path_with_query: Optional[str]) -> None:
        """
        Raises: `wit_world.types.Err(None)`
        """
        raise NotImplementedError
    def scheme(self) -> Optional[Scheme]:
        raise NotImplementedError
    def set_scheme(self, scheme: Optional[Scheme]) -> None:
        """
        Raises: `wit_world.types.Err(None)`
        """
        raise NotImplementedError
    def authority(self) -> Optional[str]:
        raise NotImplementedError
    def set_authority(self, authority: Optional[str]) -> None:
        """
        Raises: `wit_world.types.Err(None)`
        """
        raise NotImplementedError
    def headers(self) -> Fields:
        raise NotImplementedError
    def __enter__(self) -> Self:
        """Returns self"""
        return self
                                
    def __exit__(self, exc_type: type[BaseException] | None, exc_value: BaseException | None, traceback: TracebackType | None) -> bool | None:
        """
        Release this resource.
        """
        raise NotImplementedError


class RequestOptions:
    
    def __init__(self) -> None:
        raise NotImplementedError

    def connect_timeout(self) -> Optional[int]:
        raise NotImplementedError
    def set_connect_timeout(self, duration: Optional[int]) -> None:
        """
        Raises: `wit_world.types.Err(None)`
        """
        raise NotImplementedError
    def first_byte_timeout(self) -> Optional[int]:
        raise NotImplementedError
    def set_first_byte_timeout(self, duration: Optional[int]) -> None:
        """
        Raises: `wit_world.types.Err(None)`
        """
        raise NotImplementedError
    def between_bytes_timeout(self) -> Optional[int]:
        raise NotImplementedError
    def set_between_bytes_timeout(self, duration: Optional[int]) -> None:
        """
        Raises: `wit_world.types.Err(None)`
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


class OutgoingResponse:
    
    def __init__(self, headers: Fields) -> None:
        raise NotImplementedError

    def status_code(self) -> int:
        raise NotImplementedError
    def set_status_code(self, status_code: int) -> None:
        """
        Raises: `wit_world.types.Err(None)`
        """
        raise NotImplementedError
    def headers(self) -> Fields:
        raise NotImplementedError
    def body(self) -> OutgoingBody:
        """
        Raises: `wit_world.types.Err(None)`
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


class ResponseOutparam:
    
    @classmethod
    def set(cls, param: Self, response: Result[OutgoingResponse, ErrorCode]) -> None:
        raise NotImplementedError
    def __enter__(self) -> Self:
        """Returns self"""
        return self
                                
    def __exit__(self, exc_type: type[BaseException] | None, exc_value: BaseException | None, traceback: TracebackType | None) -> bool | None:
        """
        Release this resource.
        """
        raise NotImplementedError


class IncomingResponse:
    
    def status(self) -> int:
        raise NotImplementedError
    def headers(self) -> Fields:
        raise NotImplementedError
    def consume(self) -> IncomingBody:
        """
        Raises: `wit_world.types.Err(None)`
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


class FutureIncomingResponse:
    
    def subscribe(self) -> poll.Pollable:
        raise NotImplementedError
    def get(self) -> Optional[Result[Result[IncomingResponse, ErrorCode], None]]:
        raise NotImplementedError
    def __enter__(self) -> Self:
        """Returns self"""
        return self
                                
    def __exit__(self, exc_type: type[BaseException] | None, exc_value: BaseException | None, traceback: TracebackType | None) -> bool | None:
        """
        Release this resource.
        """
        raise NotImplementedError



def http_error_code(err: wasi_io_error.Error) -> Optional[ErrorCode]:
    raise NotImplementedError

