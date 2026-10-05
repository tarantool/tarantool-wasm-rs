"""Owned host tuples, key definitions, formats and field iterators."""

import operator
import sys
from . import _codec
from ._resources import Resource, WitResource, same_bindings
from .database import unsigned
from .types import KeyPart, KeyPartFlags, FieldType, Collation


class Tuple(Resource):
    def _release(self, handle):
        self._bindings.call("box_tuple", "release", handle)

    def copy(self):
        handle = self._get()
        self._bindings.call("box_tuple", "retain", handle)
        return Tuple(self._bindings, handle)

    __copy__ = copy

    def __len__(self):
        return self._bindings.call("box_tuple", "field_count", self._get())

    @property
    def byte_size(self):
        return self._bindings.call("box_tuple", "byte_size", self._get())

    def to_bytes(self):
        return self._bindings.call("box_tuple", "to_bytes", self._get())

    def to_tuple(self):
        return tuple(_codec.unpack(self.to_bytes()))

    @property
    def format(self):
        return TupleFormat(
            self._bindings, self._bindings.call("box_tuple", "format", self._get())
        )

    def __getitem__(self, index):
        if isinstance(index, slice):
            return self.to_tuple()[index]
        if isinstance(index, str):
            result = self._bindings.call(
                "box_tuple", "field_by_path", self._get(), index
            )
            if result is None:
                raise KeyError(index)
        else:
            index = operator.index(index)
            if index < 0:
                index += len(self)
            if index < 0 or index >= len(self):
                raise IndexError(index)
            result = self._bindings.call("box_tuple", "field", self._get(), index)
            if result is None:
                raise IndexError(index)
        return _codec.unpack(result)

    def __iter__(self):
        # A copied iterator needs no host cleanup on an early break.
        return iter(self.to_tuple())

    def iter_fields(self):
        handle = self._bindings.invoke(
            self._bindings.module("box_tuple").Iterator.new, self._get()
        )
        return TupleIterator(self._bindings, handle)

    def update(self, operations):
        return Tuple(
            self._bindings,
            self._bindings.call(
                "box_tuple",
                "update",
                self._get(),
                _codec.operations(operations, tuple_api=True),
            ),
        )

    def upsert(self, operations):
        return Tuple(
            self._bindings,
            self._bindings.call(
                "box_tuple",
                "upsert",
                self._get(),
                _codec.operations(operations, tuple_api=True),
            ),
        )

    def validate(self, format):
        same_bindings(self, format)
        self._bindings.call("box_tuple", "validate", self._get(), format._get())

    def compare(self, other, key_def):
        same_bindings(self, other, key_def)
        return self._bindings.call(
            "box_tuple", "compare", key_def._get(), self._get(), other._get()
        )

    def compare_with_key(self, key, key_def):
        same_bindings(self, key_def)
        return self._bindings.call(
            "box_tuple",
            "compare_with_key",
            self._get(),
            _codec.key(key),
            key_def._get(),
        )


class TupleIterator(WitResource):
    def __iter__(self):
        return self

    def __next__(self):
        if self.closed:
            raise StopIteration
        try:
            result = self._bindings.invoke(self._get().next)
            if result is None:
                # Keep the cursor open: WIT allows rewind/seek after exhaustion.
                raise StopIteration
            return _codec.unpack(result)
        except StopIteration:
            raise
        except Exception:
            self.__exit__(*sys.exc_info())
            raise

    @property
    def position(self):
        return self._bindings.invoke(self._get().position)

    def rewind(self):
        self._bindings.invoke(self._get().rewind)

    def seek(self, position):
        result = self._bindings.invoke(self._get().seek, unsigned(position))
        if result is None:
            raise IndexError(position)
        return _codec.unpack(result)


class KeyDef(Resource):
    def _release(self, handle):
        self._bindings.call("key_def", "release", handle)

    def copy(self):
        return KeyDef(
            self._bindings, self._bindings.call("key_def", "duplicate", self._get())
        )

    __copy__ = copy

    def merge(self, other):
        same_bindings(self, other)
        return KeyDef(
            self._bindings,
            self._bindings.call("key_def", "merge", self._get(), other._get()),
        )

    def __len__(self):
        return self._bindings.call("key_def", "part_count", self._get())

    @property
    def parts(self):
        result = self._bindings.call("key_def", "dump_parts", self._get())
        return tuple(
            KeyPart(
                p.field_no,
                FieldType(p.field_type.name.lower().replace("_", "-")),
                (
                    None
                    if p.collation is None
                    else Collation(p.collation.name.lower().replace("_", "-"))
                ),
                p.path,
                KeyPartFlags(p.flags.value),
            )
            for p in result
        )

    def validate_key(self, key, *, full=False):
        self._bindings.call(
            "key_def",
            "validate_full_key" if full else "validate_key",
            self._get(),
            _codec.key(key),
        )

    def extract_key(self, value):
        same_bindings(self, value)
        return tuple(
            _codec.unpack(
                self._bindings.call("key_def", "extract_key", self._get(), value._get())
            )
        )

    def validate_tuple(self, value):
        same_bindings(self, value)
        self._bindings.call("key_def", "validate_tuple", self._get(), value._get())


class TupleFormat(Resource):
    def _release(self, handle):
        self._bindings.call("tuple_format", "release", handle)

    def copy(self):
        handle = self._get()
        self._bindings.call("tuple_format", "retain", handle)
        return TupleFormat(self._bindings, handle)

    __copy__ = copy


def key_part(bindings, value):
    if not isinstance(value, KeyPart):
        raise TypeError("expected KeyPart")
    flags = KeyPartFlags(value.flags)
    if flags.value & ~3:
        raise ValueError("unsupported key-part flags")
    generated = bindings.module("types").KeyPartFlags
    converted = generated(0)
    for flag in (KeyPartFlags.IS_NULLABLE, KeyPartFlags.EXCLUDE_NULL):
        if flags & flag:
            converted |= generated[flag.name]
    return bindings.record(
        "KeyPart",
        unsigned(value.field_no),
        bindings.enum("FieldType", FieldType(value.field_type)),
        (
            None
            if value.collation is None
            else bindings.enum("Collation", Collation(value.collation))
        ),
        value.path,
        converted,
    )
