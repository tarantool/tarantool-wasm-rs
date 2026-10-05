"""Raw componentize-py bindings. No value, error or ownership conversion.

For custom generated names call configure(database=..., types=..., ...)
before accessing bindings. High-level clients with explicit bindings are isolated.
"""

# ruff: noqa: F822 -- interface exports are resolved by module __getattr__.

from importlib import import_module

__all__ = [
    "types",
    "database",
    "index",
    "box_tuple",
    "key_def",
    "tuple_format",
    "transaction",
    "sequence",
    "session",
    "error",
    "log",
    "metrics",
    "msgpack",
    "lua_callback",
]
_modules = {}
_markers = {"types": "BoxTuple", "error": "last"}


class _UnavailableInterface(RuntimeError):
    pass


def configure(**modules):
    unknown = modules.keys() - set(__all__)
    if unknown:
        raise ValueError(f"unknown interfaces: {sorted(unknown)}")
    _modules.update(modules)


def __getattr__(name):
    if name not in __all__:
        raise AttributeError(name)
    if name not in _modules:
        candidates = (
            (f"docs_adder_{name}", name)
            if name == "lua_callback"
            else (f"tarantool_tarantool_{name}", name)
        )
        for candidate in candidates:
            path = f"wit_world.imports.{candidate}"
            try:
                module = import_module(path)
            except ModuleNotFoundError as error:
                if error.name not in (path, "wit_world", "wit_world.imports"):
                    raise
            else:
                marker = _markers.get(name)
                if marker is not None and not hasattr(module, marker):
                    continue
                _modules[name] = module
                break
        else:
            raise _UnavailableInterface(
                f"WIT interface {name!r} is unavailable; add its import to the world "
                "or configure _tarantool with generated modules"
            )
    return _modules[name]


# componentize-py snapshots imported modules; runtime importlib cannot load
# generated sources which were never frozen. Resolve the interfaces present
# in this world now, without requiring absent interfaces or calling the host.
for _name in __all__:
    try:
        __getattr__(_name)
    except _UnavailableInterface:
        pass
del _name
