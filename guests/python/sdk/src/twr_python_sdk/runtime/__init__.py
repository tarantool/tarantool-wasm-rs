"""Explicit guest initialization, before importing standard-library consumers."""
from pathlib import Path
import sys


def install(ports=("ssl", "asyncio_dns")):
    """Install selected runtime ports; an ordinary SDK import changes no modules."""
    if isinstance(ports, str):
        raise TypeError("ports must be a sequence of port names")
    ports = tuple(ports)
    unknown = set(ports) - {"ssl", "asyncio_dns"}
    if unknown:
        raise ValueError("unknown runtime ports: " + ", ".join(sorted(unknown)))
    if not ports:
        return
    guest = str(Path(__file__).resolve().parents[1] / "_guest")
    if guest not in sys.path:
        sys.path.insert(0, guest)
    if "ssl" in ports:
        import tarantool_ssl
        tarantool_ssl.install()
        # Include stdlib consumers used by dynamically imported packages.
        import http.client
        import http.cookiejar
        import http.cookies
        import urllib.request
        import asyncio.sslproto
    if "asyncio_dns" in ports:
        import tarantool_asyncio
        tarantool_asyncio.install()
