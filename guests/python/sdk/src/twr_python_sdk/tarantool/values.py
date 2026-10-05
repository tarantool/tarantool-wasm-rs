"""Values which cannot be represented losslessly by Python's built-in types."""

from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
from enum import IntEnum

_EPOCH = datetime(1970, 1, 1, tzinfo=timezone.utc)


def integer(name, value, minimum, maximum):
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{name} must be an integer")
    if not minimum <= value <= maximum:
        raise ValueError(f"{name} must be in [{minimum}, {maximum}]")


@dataclass(frozen=True)
class Datetime:
    """Tarantool datetime, including nanoseconds and its numeric timezone ID.

    seconds are UTC Unix seconds; tzoffset is in minutes. No timezone database
    is needed to preserve tzindex. Python conversions use a fixed UTC offset.
    """

    seconds: int
    nanoseconds: int = 0
    tzoffset: int = 0
    tzindex: int = 0

    def __post_init__(self):
        integer("seconds", self.seconds, -185604722870400, 185480451417600)
        # Tarantool reserves the second billion for leap seconds.
        integer("nanoseconds", self.nanoseconds, 0, 1999999999)
        integer("tzoffset", self.tzoffset, -720, 840)
        integer("tzindex", self.tzindex, 0, 1024)

    @classmethod
    def from_datetime(cls, value):
        if not isinstance(value, datetime):
            raise TypeError("expected datetime.datetime")
        offset = value.utcoffset()
        if offset is None:
            raise ValueError("datetime must have an explicit timezone")
        if offset % timedelta(minutes=1):
            raise ValueError("Tarantool timezone offsets must be whole minutes")
        # A valid local year 1/9999 may have a UTC instant outside Python's
        # range. Keep the intermediate as a timedelta, not a UTC datetime.
        delta = value.replace(tzinfo=None) - _EPOCH.replace(tzinfo=None) - offset
        return cls(
            delta.days * 86400 + delta.seconds,
            delta.microseconds * 1000,
            offset // timedelta(minutes=1),
        )

    def to_datetime(self, *, truncate=False):
        if self.nanoseconds >= 1000000000:
            raise ValueError("Python datetime cannot represent a leap second")
        if self.nanoseconds % 1000 and not truncate:
            raise ValueError("nanoseconds would be lost; pass truncate=True explicitly")
        zone = timezone(timedelta(minutes=self.tzoffset))
        local = _EPOCH.replace(tzinfo=None) + timedelta(
            seconds=self.seconds + self.tzoffset * 60,
            microseconds=self.nanoseconds // 1000,
        )
        return local.replace(tzinfo=zone)


class IntervalAdjust(IntEnum):
    EXCESS = 0
    LIMIT = 1
    SNAP = 2


@dataclass(frozen=True)
class Interval:
    """Calendar interval; months/years stay separate from elapsed seconds."""

    years: int = 0
    months: int = 0
    weeks: int = 0
    days: int = 0
    hours: int = 0
    minutes: int = 0
    seconds: int = 0
    nanoseconds: int = 0
    adjust: IntervalAdjust = IntervalAdjust.LIMIT

    def __post_init__(self):
        for name in ("years", "months", "weeks", "nanoseconds"):
            integer(name, getattr(self, name), -(2**31), 2**31 - 1)
        # The corresponding host fields are doubles. Restrict to the contiguous
        # exact integer range to avoid a silent rounding during host decoding.
        for name in ("days", "hours", "minutes", "seconds"):
            integer(name, getattr(self, name), -(2**53), 2**53)
        integer("adjust", self.adjust, 0, 2)
        object.__setattr__(self, "adjust", IntervalAdjust(self.adjust))


@dataclass(frozen=True)
class ErrorFrame:
    """One frame of a MessagePack error value, separate from WIT failures."""

    code: int
    message: str
    error_type: str = "ClientError"
    file: str = ""
    line: int = 0
    errno: int = 0
    fields: dict = field(default_factory=dict)
    extra: dict = field(default_factory=dict)

    def __post_init__(self):
        for name in ("code", "line", "errno"):
            integer(name, getattr(self, name), 0, 2**32 - 1)
        for name in ("message", "error_type", "file"):
            if not isinstance(getattr(self, name), str):
                raise TypeError(f"{name} must be a string")
        if not isinstance(self.fields, dict) or not all(
            isinstance(key, str) for key in self.fields
        ):
            raise TypeError("error fields must be a dict with string keys")
        _extra(self.extra, range(7))


@dataclass(frozen=True)
class ErrorValue:
    """An MP_ERROR value; decoding it does not raise a Python exception."""

    stack: tuple[ErrorFrame, ...]
    extra: dict = field(default_factory=dict)

    def __post_init__(self):
        stack = tuple(self.stack)
        if not stack or not all(isinstance(frame, ErrorFrame) for frame in stack):
            raise TypeError("error stack must contain at least one ErrorFrame")
        object.__setattr__(self, "stack", stack)
        _extra(self.extra, (0,))


def _extra(value, reserved):
    if not isinstance(value, dict):
        raise TypeError("extra error attributes must be a dict")
    for key in value:
        integer("extra error key", key, 0, 2**64 - 1)
        if key in reserved:
            raise ValueError("extra error attributes cannot override known fields")
