# Platform compatibility

Linux release builds target x86-64 with Rust 1.96.0 and GLIBC >= 2.28.
Tarantool testing covers versions >= 3.6; binaries do not require an exact
Tarantool patch version. macOS release builds target arm64.

## Supported matrix

| Distribution | Test environment |
| --- | --- |
| Ubuntu 22.04 | `tarantool/testing:ubuntu-jammy` |
| Debian 12 | `packpack/packpack:debian-bookworm` |
| Fedora 39 | `packpack/packpack:fedora-39` |
| CentOS 8 | `packpack/packpack:centos-8` |
| RED OS 7.3.4 | `packpack/packpack:redos-7.3` |
| Astra Linux 1.7 | `packpack/packpack:astra-1.7` |

The Linux release library is built on CentOS 8 and tested unchanged in each
environment above. Tests include Python startup and non-SSL calls with
unavailable or incompatible OpenSSL, plus SSL calls with OpenSSL 3 installed.

## Binary compatibility

A current Ubuntu, Debian, or Fedora build can require newer GLIBC symbols than
older supported systems provide. The portable Linux library is built in EL8
and requires no GLIBC symbols newer than 2.28. OpenSSL is loaded dynamically
when Python SSL calls need it.
