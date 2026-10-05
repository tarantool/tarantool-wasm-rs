"""Spaces and indexes accept Python values and return copied tuples."""

import operator
import sys
from . import _codec
from ._resources import WitResource
from .types import IteratorType


def row(bindings, handle):
    if handle is None:
        return None
    try:
        value = tuple(_codec.unpack(bindings.call("box_tuple", "to_bytes", handle)))
    except BaseException:
        try:
            bindings.call("box_tuple", "release", handle)
        except Exception:
            pass
        raise
    else:
        bindings.call("box_tuple", "release", handle)
        return value


def unsigned(value, bits=32):
    value = operator.index(value)
    if not 0 <= value < 1 << bits:
        raise ValueError(f"expected an unsigned {bits}-bit integer")
    return value


class IndexIterator(WitResource):
    def __iter__(self):
        return self

    def __next__(self):
        if self.closed:
            raise StopIteration
        try:
            handle = self._bindings.invoke(self._get().next)
            if handle is None:
                self.close()
                raise StopIteration
            return row(self._bindings, handle)
        except StopIteration:
            raise
        except Exception:
            self.__exit__(*sys.exc_info())
            raise


class Index:
    def __init__(self, bindings, space, index):
        self._bindings = bindings
        self.space = space
        self._index = index

    @property
    def id(self):
        return self._resolve().id

    def _resolve(self):
        space = self.space._resolve()
        if isinstance(self._index, str):
            index = self._bindings.call("database", "index_by_name", space, self._index)
            if index is None:
                raise KeyError(self._index)
            return index
        return self._bindings.record("Index", unsigned(self._index), space.id)

    def get(self, key):
        return row(
            self._bindings,
            self._bindings.call("index", "get", self._resolve(), _codec.key(key)),
        )

    def min(self, key=()):
        return row(
            self._bindings,
            self._bindings.call("index", "min", self._resolve(), _codec.key(key)),
        )

    def max(self, key=()):
        return row(
            self._bindings,
            self._bindings.call("index", "max", self._resolve(), _codec.key(key)),
        )

    def random(self, seed):
        return row(
            self._bindings,
            self._bindings.call("index", "random", self._resolve(), unsigned(seed)),
        )

    def __len__(self):
        return self._bindings.call("index", "tuple_count", self._resolve())

    @property
    def memory_size(self):
        return self._bindings.call("index", "memory_size", self._resolve())

    def count(self, key=(), *, mode=IteratorType.EQUAL):
        return self._bindings.call(
            "index",
            "count",
            self._resolve(),
            self._bindings.enum("IteratorType", IteratorType(mode)),
            _codec.key(key),
        )

    def iter(self, key=(), *, mode=None):
        if mode is None:
            mode = (
                IteratorType.ALL
                if isinstance(key, (tuple, list)) and not key
                else IteratorType.EQUAL
            )
        handle = self._bindings.invoke(
            self._bindings.module("index").Iterator.new,
            self._resolve(),
            self._bindings.enum("IteratorType", IteratorType(mode)),
            _codec.key(key),
        )
        return IndexIterator(self._bindings, handle)

    def select(self, key=(), *, mode=None, limit=None, offset=0):
        offset = unsigned(offset, 64)
        if limit is not None:
            limit = unsigned(limit, 64)
        result = []
        with self.iter(key, mode=mode) as iterator:
            for _ in range(offset):
                if next(iterator, None) is None:
                    return result
            while limit is None or len(result) < limit:
                item = next(iterator, None)
                if item is None:
                    break
                result.append(item)
        return result

    def update(self, key, operations):
        return row(
            self._bindings,
            self._bindings.call(
                "database",
                "update",
                self._resolve(),
                _codec.key(key),
                _codec.operations(operations),
            ),
        )

    def upsert(self, value, operations):
        self._bindings.call(
            "database",
            "upsert",
            self._resolve(),
            _codec.array(value),
            _codec.operations(operations),
        )

    def delete(self, key):
        return row(
            self._bindings,
            self._bindings.call("database", "delete", self._resolve(), _codec.key(key)),
        )


class Indexes:
    def __init__(self, space):
        self._space = space

    def __getitem__(self, index):
        value = Index(self._space._bindings, self._space, index)
        value._resolve()
        return value


class Space:
    def __init__(self, bindings, space):
        self._bindings = bindings
        self._space = space
        self.index = Indexes(self)

    def _resolve(self):
        if isinstance(self._space, str):
            result = self._bindings.call("database", "space_by_name", self._space)
            if result is None:
                raise KeyError(self._space)
            return result
        return self._bindings.record("Space", unsigned(self._space))

    @property
    def id(self):
        return self._resolve().id

    def insert(self, value):
        return row(
            self._bindings,
            self._bindings.call(
                "database", "insert", self._resolve(), _codec.array(value)
            ),
        )

    def replace(self, value):
        return row(
            self._bindings,
            self._bindings.call(
                "database", "replace", self._resolve(), _codec.array(value)
            ),
        )

    def truncate(self):
        self._bindings.call("database", "truncate", self._resolve())

    def get(self, key):
        return self.index[0].get(key)

    def update(self, key, operations):
        return self.index[0].update(key, operations)

    def upsert(self, value, operations):
        return self.index[0].upsert(value, operations)

    def delete(self, key):
        return self.index[0].delete(key)

    def select(self, key=(), **options):
        return self.index[0].select(key, **options)

    def iter(self, key=(), **options):
        return self.index[0].iter(key, **options)


class Spaces:
    def __init__(self, bindings):
        self._bindings = bindings

    def __getitem__(self, space):
        result = Space(self._bindings, space)
        result._resolve()
        return result
