"""One wrapper owns one host reference; close is idempotent."""

from .errors import ClosedResourceError


class Resource:
    def __init__(self, bindings, handle):
        self._bindings = bindings
        self._handle = handle

    @property
    def closed(self):
        return self._handle is None

    def _get(self):
        if self.closed:
            raise ClosedResourceError(f"{type(self).__name__} is closed")
        return self._handle

    def __copy__(self):
        raise TypeError(f"{type(self).__name__} cannot be copied")

    def __deepcopy__(self, memo):
        raise TypeError(
            "host resources cannot be deep-copied; copy their Python values instead"
        )

    def __reduce_ex__(self, protocol):
        raise TypeError("host resources cannot be pickled")

    def close(self):
        if not self.closed:
            handle = self._handle
            self._handle = None
            self._release(handle)

    def __enter__(self):
        self._get()
        return self

    def __del__(self):
        try:
            self.close()
        except Exception:
            pass

    def __exit__(self, exc_type, exc, traceback):
        if exc is None:
            self.close()
        else:
            try:
                self.close()
            except Exception:
                # Cleanup must not replace the application exception.
                pass
        return False


class WitResource(Resource):
    def _release(self, handle):
        handle.__exit__(None, None, None)


def same_bindings(owner, *resources):
    for resource in resources:
        if resource._bindings is not owner._bindings:
            raise ValueError("resources belong to different Tarantool bindings")
        resource._get()
