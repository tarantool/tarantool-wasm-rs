#!/bin/sh
set -eu

# Run as root inside CI/build containers. An optional command inherits the
# selected OpenSSL paths, including the runtime loader path for SSL tests.
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

case "$(pkg-config --modversion openssl)" in
    3.*) ;;
    *)
        # Older distributions only package OpenSSL 1.1. Keep it intact and
        # prepare OpenSSL 3 separately for the build and its runtime tests.
        openssl_version=3.5.9
        openssl_sha256=603f5602e2eef00d77fbd429d34dcd5822bb301757a1bc9cdb24c670f1eb859a
        openssl_prefix="/opt/twr-openssl-$openssl_version"
        if command -v apt-get >/dev/null 2>&1; then
            apt-get install -y gcc make perl curl ca-certificates
        elif command -v dnf >/dev/null 2>&1; then
            dnf --disablerepo=docker install -y gcc make perl-core curl ca-certificates
        else
            yum --disablerepo=docker install -y gcc make perl-core curl ca-certificates
        fi

        openssl_build_dir=$(mktemp -d)
        trap 'rm -rf -- "$openssl_build_dir"' 0
        trap 'exit 1' HUP INT TERM
        curl --fail --location --retry 3 \
            "https://github.com/openssl/openssl/releases/download/openssl-$openssl_version/openssl-$openssl_version.tar.gz" \
            -o "$openssl_build_dir/openssl.tar.gz"
        printf '%s  %s\n' "$openssl_sha256" "$openssl_build_dir/openssl.tar.gz" \
            | sha256sum --check -
        tar -xzf "$openssl_build_dir/openssl.tar.gz" -C "$openssl_build_dir"
        (
            cd "$openssl_build_dir/openssl-$openssl_version"
            # Do not inherit the distribution's OpenSSL 1.1 TLS policy.
            ./Configure --prefix="$openssl_prefix" --libdir=lib \
                --openssldir="$openssl_prefix/ssl" shared no-tests
            make -j "${TWR_OPENSSL_BUILD_JOBS:-2}" build_sw
            make install_sw install_ssldirs
        )
        rm -rf -- "$openssl_build_dir"
        trap - 0 HUP INT TERM

        export OPENSSL_DIR="$openssl_prefix"
        export PKG_CONFIG_PATH="$openssl_prefix/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
        export LD_LIBRARY_PATH="$openssl_prefix/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
        ;;
esac

pkg-config --modversion openssl

if [ "$#" -gt 0 ]; then
    exec "$@"
fi
