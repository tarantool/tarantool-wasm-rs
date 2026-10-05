"""Python guest API and standard-library ports for Tarantool-Wasm-Rs."""
from .runtime import install

__version__ = "0.1.0"
__all__ = ["install", "freeze_packages", "patch_socket_errors"]


def freeze_packages(*names):
    """Freeze portable package sources for dynamic imports in the component."""
    from .runtime.freeze_packages import freeze_packages as freeze
    return freeze(*names)


def patch_socket_errors(component):
    """Correct the pinned wasi-libc mapper in a stripped componentize-py component."""
    from ._guest.patch_wasi_socket_errors import patch_file
    patch_file(component)
