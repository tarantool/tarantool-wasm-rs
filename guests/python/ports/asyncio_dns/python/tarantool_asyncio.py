"""Use WASI's socket resolver where Python cannot start executor threads."""

import sys


async def _getaddrinfo(self, host, port, *, family=0, type=0, proto=0, flags=0):
    import asyncio
    import socket

    self._check_closed()
    # Give queued work/cancellation a turn before entering the blocking WASI
    # resolver. DNS itself does not run concurrently with other guest tasks.
    await asyncio.sleep(0)
    resolver = self._getaddrinfo_debug if self.get_debug() else socket.getaddrinfo
    return resolver(host, port, family, type, proto, flags)


def install():
    """Adapt stdlib loops only on WASI; retain custom loop overrides."""
    if sys.platform != "wasi":
        return False
    from asyncio import BaseEventLoop

    BaseEventLoop.getaddrinfo = _getaddrinfo
    return True
