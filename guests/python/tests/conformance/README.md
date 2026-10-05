# CPython SSL conformance

`upstream/cpython-3.12.11/` contains unmodified CPython 3.12.11 `test_ssl.py`,
its required `test.support`/server helpers, certificate fixtures and PSF license.
`selection.json` pins every source/fixture hash and accounts for all 183 methods.

The Wasm subset selects 86 original methods with no assertion/decorator changes.
It requires 86 PASS and zero errors, failures, skips or expected failures.
The remaining 97 methods are excluded from that guest run and never counted as
PASS:

| Reason | Methods | Requirement |
| --- | --- | --- |
| Python threads | 90 | Original threaded server/helpers unavailable in the guest |
| Windows | 3 | Windows certificate store/platform |
| Locale | 1 | Required nondefault locale |
| Timezone | 1 | Mutable timezone support |
| External network | 2 | Upstream internet endpoints/resources |

The separate complete native run uses the original module and lifecycle hooks
on CPython 3.12.11, with regrtest's default resource selection. Native PASS/SKIP
results are reported separately; they do not establish guest conformance.
The guest still runs embedded CPython 3.14.0 from componentize-py 0.25.0.

## Run

From the repository root, using CPython 3.12.11:

```sh
python3.12 guests/python/tests/conformance/generate.py > /tmp/ssl-native.json
python3.12 guests/python/tests/conformance/native.py > /tmp/ssl-native-full.json
bash guests/python/tests/components/ssl_conformance/build.sh
TWR_SSL_CONFORMANCE_REPORT=/tmp/ssl-guest.json bash guests/python/test.sh
```

The build installs the SDK wheel outside the checkout in
`mandeser0/tarawasm:v0.3.0`, freezes the generated module/helpers and strips and
validates the component. Lua grants loopback-only networking, read-only upstream
fixtures at `/ssl-conformance` and a writable temporary preopen at `/tmp`.

Reports include actual runtime/OpenSSL versions, source/fixture hashes, method
IDs and diagnostics. `guests-python.yml` preserves selected native, full native
and guest JSON reports. An independent deliberately failed ordinary unittest
assertion verifies that frozen WIT exceptions do not break traceback cleanup;
this probe is outside the upstream pass count.

## Extend

Add unchanged method IDs/helpers and update their exclusion entries in
`selection.json`. Lifecycle hooks must be explicitly included. Pin any added
original support/fixture files. Unit checks compare generated ASTs, reject
missing selections and detect tampering. Fix the adapter or prerequisites;
keep unsupported scenarios classified rather than weakening assertions.
