"""Independent component: public HTTPS API with a host SSL WIT import."""
import twr_python_sdk
twr_python_sdk.install()



import ssl
import urllib.request
from wit_world import Err

# This also checks that ordinary context construction is safe while the
# componentizer snapshots the application, before host imports are available.
IMPORT_CONTEXT = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)


class WitWorld:
    def fetch(self, url, cafile):
        try:
            context = ssl.create_default_context(cafile=cafile)
            with urllib.request.urlopen(url, context=context, timeout=3) as response:
                return response.read().decode('utf-8')
        except Exception as error:
            cause = error.reason if isinstance(error, urllib.error.URLError) else error
            reason = getattr(cause, 'reason', None)
            raise Err(f'{type(cause).__name__}: {reason or str(cause)}') from None
