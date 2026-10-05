#!/bin/sh
set -eu

# Run as root inside CI/build containers. The test containers also load the
# native module, so they need the matching OpenSSL shared libraries.
if command -v apt-get >/dev/null 2>&1; then
    apt-get update
    apt-get install -y pkg-config libssl-dev
elif command -v dnf >/dev/null 2>&1; then
    dnf --disablerepo=docker install -y pkgconf-pkg-config openssl-devel
elif command -v yum >/dev/null 2>&1; then
    yum --disablerepo=docker install -y pkgconfig openssl-devel
else
    echo 'An apt-get, dnf, or yum package manager is required for OpenSSL dependencies.' >&2
    exit 1
fi

pkg-config --modversion openssl
