# Public TLS fixtures

These keys/certificates are public local-test data. `server-encrypted.key` and
`weak-rsa-encrypted.key` use password `fixture-password`.

| Files | Purpose |
| --- | --- |
| `server.*`, `ca.crt`, `leaf.crt`, `client-leaf.crt` | Server/client verification and mutual TLS |
| `decoded.crt`, `decoded.cnf`, `decoded.json` | CPython formatting of RDNs, Unicode, OIDs, SANs and URLs |
| `common-name.crt` | Hostname Common Name fallback |
| `empty.crl`, `revoked.crl` | CRL checks; revoked leaf serial `82` |
| `rejected-ca.pem`, `padded-store.pem` | AUX trust restrictions and delimiter whitespace |
| `ec.*`, `weak-rsa.*`, `weak-chain.pem` | Key-type selection and security levels |
| `dh2048.pem`, `dh1024.pem` | Accepted/rejected public DH parameters |
| `ciphers.json` | CPython 3.12/OpenSSL reference cipher metadata |

Certificate URLs use `.test` and are not fetched by the decoder. Fixtures have
fixed validity dates; regenerating certificates changes those dates and requires
updating dependent reference JSON, chains and CRLs together.

Run regeneration from this directory. For the encrypted server key:

```sh
openssl pkcs8 -topk8 -in server.key -out server-encrypted.key \
  -v2 aes-256-cbc -passout pass:fixture-password
```

`decoded.cnf`/`leaf.cnf` define extensions. Regenerate `decoded.json` from the
replacement certificate using the pinned CPython 3.12.11 reference:

```sh
python3.12 - <<'PYTHON'
import json, ssl
from pathlib import Path
Path('decoded.json').write_text(json.dumps(
    ssl._ssl._test_decode_cert('decoded.crt'), ensure_ascii=False, indent=2) + '\n')
PYTHON
```

The pinned upstream CPython fixtures live separately in
`tests/conformance/upstream/` and must retain their `selection.json` checksums.
