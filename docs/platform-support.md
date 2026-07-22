# Platform compatibility

The supported release matrix targets x86-64 with Rust 1.94.0 and Tarantool
3.7.0. The Linux checks were reproduced on 2026-07-22 on an x86-64 Ubuntu
Docker host.

Run the complete Linux release matrix with:

```sh
./scripts/test_distros.sh
```

The script mounts the source read-only and gives each distribution separate
Docker build and test volumes. This prevents native artifacts from different
libc versions from being reused accidentally. The script uses
`cargo build --locked --release` and then runs the full test suite.

## Supported matrix

| Distribution | Test environment | Release build | Full `luatest` suite | Highest required GLIBC symbol | Status |
| --- | --- | --- | --- | --- | --- |
| Debian 12 | `packpack/packpack:debian-bookworm` | PASS | PASS | `GLIBC_2.34` | Supported |
| Fedora 39 | `packpack/packpack:fedora-39` | PASS | PASS | `GLIBC_2.34` | Supported |
| CentOS 8 | `packpack/packpack:centos-8` | PASS | PASS | `GLIBC_2.28` | Supported |
| RED OS 7.3.4 | `packpack/packpack:redos-7.3` | PASS | PASS | `GLIBC_2.28` | Supported |
| Astra Linux 1.7 | `packpack/packpack:astra-1.7` | PASS | PASS | `GLIBC_2.28` | Supported |

Each release job installs the official Tarantool 3.7.0 runtime and development
packages, installs `luatest` through `tt`, and runs every test under `test/`
against the distribution-native `wasm.so`.

Fedora 39 and CentOS 8 are intentionally pinned to the PackPack tags available
to Tarantool CI. Updating a distribution version requires rerunning the matrix
and recording the new libc requirement here.

## Linux binary compatibility

Do not publish an artifact built on Debian 12, Fedora 39, or a current Ubuntu
runner as a generic Linux binary. Those artifacts require `GLIBC_2.34` and do
not load on the tested GLIBC 2.28 systems. A portable release artifact must be
built in an EL8 or equivalent oldest-supported userspace, then loaded on every
newer supported distribution.

The distribution workflow currently builds inside every target environment.
This proves that the source has a working release build on each system; it
does not yet assert that one shared Linux binary runs everywhere.

## CI workflows

`.github/workflows/release-distributions.yaml` builds and fully tests each
Linux distribution independently. Per-job Docker volumes avoid cross-job
native artifact reuse and are removed even after failures.
