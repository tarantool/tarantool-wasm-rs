"""Embedded stdlib support without the SDK's SSL interface."""
import sys
_ssl_before = sys.modules.get("ssl")
import twr_python_sdk
assert sys.modules.get("ssl") is _ssl_before
import zlib


class WitWorld:
    def round_trip(self, data):
        return zlib.decompress(zlib.compress(data))
