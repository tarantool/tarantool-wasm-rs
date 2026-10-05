"""Snapshot pure Python packages without executing their imports at build time."""
import importlib.abc
import importlib.machinery
import importlib.util
try:
    from importlib.resources.abc import Traversable, TraversableResources
except ImportError:  # Python 3.10 build environments.
    from importlib.abc import Traversable, TraversableResources
import io
import os
import posixpath
import sys

_LIMIT = 16 * 1024 * 1024


class _Resource(Traversable):
    def __init__(self, path, files):
        self.path, self._files = path, files

    @property
    def name(self):
        return posixpath.basename(self.path)

    def is_file(self):
        return self.path in self._files

    def is_dir(self):
        prefix = self.path + "/"
        return any(path.startswith(prefix) for path in self._files)

    def iterdir(self):
        if not self.is_dir():
            raise NotADirectoryError(self.path)
        prefix = self.path + "/"
        children = {path[len(prefix):].split("/", 1)[0]
                    for path in self._files if path.startswith(prefix)}
        return iter(self.joinpath(child) for child in sorted(children))

    def joinpath(self, *descendants):
        path = self.path
        for child in descendants:
            child = os.fspath(child)
            if not isinstance(child, str) or child.startswith("/") or ".." in child.split("/"):
                raise ValueError("resource path must remain inside its package")
            path = posixpath.join(path, child)
        return _Resource(posixpath.normpath(path), self._files)

    def __truediv__(self, child):
        return self.joinpath(child)

    def open(self, mode="r", *args, **kwargs):
        if mode not in ("r", "rb", "rt"):
            raise ValueError("frozen resources are read-only")
        if self.is_dir():
            raise IsADirectoryError(self.path)
        try:
            data = self._files[self.path]
        except KeyError:
            raise FileNotFoundError(self.path) from None
        stream = io.BytesIO(data)
        if mode == "rb":
            if args or kwargs:
                raise ValueError("binary resource mode does not accept text options")
            return stream
        return io.TextIOWrapper(stream, *args, **kwargs)


class _ResourceReader(TraversableResources):
    def __init__(self, path, files):
        self.path, self._files = path, files

    def files(self):
        return _Resource(self.path, self._files)


class _FrozenPackages(importlib.abc.MetaPathFinder, importlib.abc.Loader):
    def __init__(self):
        self.modules = {}
        self.resources = {}

    def find_spec(self, fullname, path=None, target=None):
        item = self.modules.get(fullname)
        if item is None:
            return None
        code, source, filename, package = item
        spec = importlib.machinery.ModuleSpec(fullname, self, origin=filename, is_package=package)
        spec.has_location = True
        if package:
            spec.submodule_search_locations = [os.path.dirname(filename)]
        return spec

    def create_module(self, spec):
        return None

    def exec_module(self, module):
        exec(self.get_code(module.__name__), module.__dict__)

    def get_code(self, fullname):
        return self.modules[fullname][0]

    def get_source(self, fullname):
        return importlib.util.decode_source(self.modules[fullname][1])

    def get_filename(self, fullname):
        return self.modules[fullname][2]

    def is_package(self, fullname):
        return self.modules[fullname][3]

    def get_data(self, path):
        try:
            return self.resources[path]
        except KeyError:
            raise FileNotFoundError(path) from None

    def get_resource_reader(self, fullname):
        item = self.modules.get(fullname)
        if item is None or not item[3]:
            return None
        return _ResourceReader(posixpath.dirname(item[2]), self.resources)


_finder = _FrozenPackages()


def freeze_packages(*names):
    """Call during componentization, before importing each selected package.

    Sources must exist on the build Python path. Code executes on first import
    in the instantiated component. Resources are immutable memory objects;
    importlib.resources.as_file may extract them to a guest writable temp dir.
    Regular packages and standalone .py modules work; native extensions do not.
    """
    pending = {}
    resources = {}
    total = 0
    for name in names:
        if not isinstance(name, str) or not name.isidentifier():
            raise ValueError("freeze_packages requires top-level module names")
        if name in sys.modules or name in _finder.modules:
            raise ValueError("package must be frozen before it is imported: " + name)
        spec = importlib.machinery.PathFinder.find_spec(name)
        if spec is None or not spec.origin or not spec.origin.endswith(".py"):
            raise ValueError("package must have Python source: " + name)
        package = spec.submodule_search_locations is not None
        root = os.path.dirname(spec.origin)
        files = []
        if package:
            for directory, directories, filenames in os.walk(root):
                directories[:] = sorted(d for d in directories if d != "__pycache__" and not d.startswith("."))
                for filename in sorted(filenames):
                    if not filename.startswith(".") and not filename.endswith((".pyc", ".so", ".pyd", ".dylib", ".dll")):
                        files.append(os.path.join(directory, filename))
        else:
            files.append(spec.origin)
        for path in files:
            relative = os.path.relpath(path, root)
            parts = relative.split(os.sep)
            is_package = parts[-1] == "__init__.py"
            if package:
                suffix = parts[:-1] if is_package else parts[:-1] + [parts[-1][:-3]]
                fullname = ".".join([name] + suffix)
                filename = "/frozen-packages/" + name + "/" + "/".join(parts)
            else:
                fullname = name
                filename = "/frozen-packages/" + name + ".py"
            with open(path, "rb") as source_file:
                source = source_file.read(_LIMIT + 1)
            total += len(source)
            if total > _LIMIT:
                raise ValueError("frozen Python source/resource limit exceeded")
            resources[filename] = source
            if path.endswith(".py"):
                pending[fullname] = (compile(source, filename, "exec", dont_inherit=True),
                                     source, filename, is_package)
    _finder.modules.update(pending)
    _finder.resources.update(resources)
    if _finder not in sys.meta_path:
        sys.meta_path.insert(0, _finder)
