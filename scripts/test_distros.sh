#!/usr/bin/env bash
set -euo pipefail

readonly RUST_VERSION="${RUST_VERSION:-1.96.0}"
readonly TARANTOOL_VERSION="${TARANTOOL_VERSION:-3.7.0}"
readonly TT_VERSION="${TT_VERSION:-2.13.0}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
SOURCE_DIR="$(cd "$SCRIPT_DIR/.." && pwd -P)"
readonly SOURCE_DIR
readonly CACHE_DIR="${DISTRO_TEST_CACHE_DIR:-$HOME/.cache/tarantool-wasm-rs/distro-test}"
readonly RUSTUP_VOLUME="${DISTRO_TEST_RUSTUP_VOLUME:-twasm-rustup}"
readonly CARGO_VOLUME="${DISTRO_TEST_CARGO_VOLUME:-twasm-cargo}"
readonly VOLUME_PREFIX="${DISTRO_TEST_VOLUME_PREFIX:-twasm}"

usage() {
    echo "Usage: $0 [debian-bookworm fedora-39 centos-8 redos-7.3 astra-1.7]"
}

image_for() {
    case "$1" in
        debian-bookworm) echo 'packpack/packpack:debian-bookworm' ;;
        fedora-39) echo 'packpack/packpack:fedora-39' ;;
        centos-8) echo 'packpack/packpack:centos-8' ;;
        redos-7.3) echo 'packpack/packpack:redos-7.3' ;;
        astra-1.7) echo 'packpack/packpack:astra-1.7' ;;
        *) echo "Unsupported distribution: $1" >&2; return 1 ;;
    esac
}

package_kind_for() {
    case "$1" in
        debian-bookworm|astra-1.7) echo deb ;;
        fedora-39|centos-8|redos-7.3) echo rpm ;;
        *) return 1 ;;
    esac
}

download() {
    local url="$1"
    local destination="$2"

    if [[ -s "$destination" ]]; then
        return
    fi

    echo "Downloading $(basename "$destination")"
    curl --fail --location --retry 3 --output "$destination.tmp" "$url"
    mv "$destination.tmp" "$destination"
}

ensure_toolchain() {
    local bootstrap_image='packpack/packpack:debian-bookworm'

    docker volume create "$RUSTUP_VOLUME" >/dev/null
    docker volume create "$CARGO_VOLUME" >/dev/null
    docker pull "$bootstrap_image" >/dev/null

    if docker run --rm --entrypoint /bin/sh \
        -e RUSTUP_HOME=/opt/rustup \
        -e CARGO_HOME=/opt/cargo \
        -e PATH=/opt/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
        -v "$RUSTUP_VOLUME:/opt/rustup" \
        -v "$CARGO_VOLUME:/opt/cargo" \
        "$bootstrap_image" \
        -c "rustc --version | grep -F 'rustc $RUST_VERSION '" >/dev/null 2>&1; then
        return
    fi

    docker run --rm --entrypoint /bin/sh \
        -e RUSTUP_HOME=/opt/rustup \
        -e CARGO_HOME=/opt/cargo \
        -v "$RUSTUP_VOLUME:/opt/rustup" \
        -v "$CARGO_VOLUME:/opt/cargo" \
        "$bootstrap_image" \
        -c "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain '$RUST_VERSION'"
}

ensure_wit_dependencies() {
    local bootstrap_image='packpack/packpack:debian-bookworm'

    git -C "$SOURCE_DIR" submodule update --init --recursive --jobs 8 \
        third-party/wasi-tls

    echo '==> Fetching WIT dependencies'
    docker run --rm --entrypoint /bin/sh \
        -e HOME=/opt/cargo \
        -e RUSTUP_HOME=/opt/rustup \
        -e CARGO_HOME=/opt/cargo \
        -e PATH=/opt/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
        -v "$RUSTUP_VOLUME:/opt/rustup" \
        -v "$CARGO_VOLUME:/opt/cargo" \
        -v "$SOURCE_DIR:/workspace" \
        "$bootstrap_image" \
        -c 'set -eu
            cd /workspace
            if ! command -v wkg >/dev/null 2>&1; then
                cargo install wkg --locked
            fi
            ./scripts/add_tarantool_registry.sh
            wkg wit fetch --type wasm'
}

build_for() {
    local distro="$1"
    local image="$2"
    local target_volume="$VOLUME_PREFIX-target-$distro"
    local artifact='/target/x86_64-unknown-linux-gnu/release/libtarantool_wasm_rs.so'

    docker volume create "$target_volume" >/dev/null

    echo "==> $distro: cargo build --locked --release"
    docker run --rm --entrypoint /bin/sh \
        -e RUSTUP_HOME=/opt/rustup \
        -e CARGO_HOME=/opt/cargo \
        -e PATH=/opt/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
        -e CARGO_TARGET_DIR=/target \
        -v "$RUSTUP_VOLUME:/opt/rustup" \
        -v "$CARGO_VOLUME:/opt/cargo" \
        -v "$target_volume:/target" \
        -v "$SOURCE_DIR:/workspace:ro" \
        "$image" \
        -c 'cd /workspace
            rustc --version
            cargo build --locked --release --target x86_64-unknown-linux-gnu'

    local max_glibc
    max_glibc="$(
        docker run --rm --entrypoint /bin/sh \
        -v "$target_volume:/target:ro" \
        "$image" \
        -c 'readelf --version-info "$1" | grep -o "GLIBC_[0-9.]*" | sort -Vu | tail -1' \
        -- "$artifact"
    )"
    echo "==> $distro: build PASS, max symbol version ${max_glibc:-unknown}"
}

test_for() {
    local distro="$1"
    local image="$2"
    local package_kind="$3"
    local artifact="$4"
    local target_volume="$VOLUME_PREFIX-target-$distro"
    local test_volume="$VOLUME_PREFIX-test-$distro"
    local package
    local devel_package

    if [[ "$package_kind" == deb ]]; then
        package="tarantool_${TARANTOOL_VERSION}-1_amd64.deb"
        devel_package="tarantool-dev_${TARANTOOL_VERSION}-1_amd64.deb"
    else
        package="tarantool-${TARANTOOL_VERSION}-1.x86_64.rpm"
        devel_package="tarantool-devel-${TARANTOOL_VERSION}-1.x86_64.rpm"
    fi

    docker volume create "$test_volume" >/dev/null

    echo "==> $distro: full luatest suite"
    docker run --rm --entrypoint /bin/sh \
        -e TARANTOOL_PACKAGE_KIND="$package_kind" \
        -e TARANTOOL_PACKAGE="/packages/$package" \
        -e TARANTOOL_DEVEL_PACKAGE="/packages/$devel_package" \
        -e TT_ARCHIVE="/packages/tt-${TT_VERSION}-linux-amd64.tar.gz" \
        -e TWASM_MODULE_DIR=/run-tests/.rocks/lib/tarantool \
        -e TWASM_ARTIFACT="$artifact" \
        -v "$SOURCE_DIR:/workspace:ro" \
        -v "$CACHE_DIR:/packages:ro" \
        -v "$target_volume:/target:ro" \
        -v "$test_volume:/run-tests" \
        "$image" \
        -c 'set -eu
            if ! command -v cmake >/dev/null 2>&1; then
                if ! command -v dnf >/dev/null 2>&1; then
                    echo "cmake is required to install luatest" >&2
                    exit 1
                fi
                dnf --disablerepo=docker install -y cmake >/dev/null
            fi
            if [ "$TARANTOOL_PACKAGE_KIND" = deb ]; then
                dpkg -i \
                    "$TARANTOOL_PACKAGE" "$TARANTOOL_DEVEL_PACKAGE" \
                    >/dev/null
            else
                rpm -Uvh --replacepkgs \
                    "$TARANTOOL_PACKAGE" "$TARANTOOL_DEVEL_PACKAGE" \
                    >/dev/null
            fi
            mkdir -p "$TWASM_MODULE_DIR"
            ln -sf "$TWASM_ARTIFACT" "$TWASM_MODULE_DIR/wasm.so"
            tar -xzf "$TT_ARCHIVE" -C /usr/local/bin tt
            cd /run-tests
            rm -rf -- /run-tests/test
            cp -a /workspace/test /run-tests/test
            rm -rf -- /run-tests/python-interpreter
            cp -a /workspace/python-interpreter /run-tests/python-interpreter
            tt rocks install luatest
            PATH="/run-tests/.rocks/bin:$PATH" \
                luatest -v -l /run-tests/luatest.log'
}

if [[ "${1:-}" == -h || "${1:-}" == --help ]]; then
    usage
    exit 0
fi

if (($# == 0)); then
    set -- debian-bookworm fedora-39 centos-8 redos-7.3 astra-1.7
fi

if [[ "$(docker info --format '{{.Architecture}}')" != x86_64 ]]; then
    echo 'This matrix currently supports Docker x86_64 only.' >&2
    exit 1
fi

mkdir -p "$CACHE_DIR"
ensure_toolchain
ensure_wit_dependencies

download \
    "https://rws.tarantool.org/release/series-3/linux-deb/pool/static/main/t/tarantool/tarantool_${TARANTOOL_VERSION}-1_amd64.deb" \
    "$CACHE_DIR/tarantool_${TARANTOOL_VERSION}-1_amd64.deb"
download \
    "https://rws.tarantool.org/release/series-3/linux-rpm/static/x86_64/Packages/tarantool-${TARANTOOL_VERSION}-1.x86_64.rpm" \
    "$CACHE_DIR/tarantool-${TARANTOOL_VERSION}-1.x86_64.rpm"
download \
    "https://rws.tarantool.org/release/series-3/linux-deb/pool/static/main/t/tarantool/tarantool-dev_${TARANTOOL_VERSION}-1_amd64.deb" \
    "$CACHE_DIR/tarantool-dev_${TARANTOOL_VERSION}-1_amd64.deb"
download \
    "https://rws.tarantool.org/release/series-3/linux-rpm/static/x86_64/Packages/tarantool-devel-${TARANTOOL_VERSION}-1.x86_64.rpm" \
    "$CACHE_DIR/tarantool-devel-${TARANTOOL_VERSION}-1.x86_64.rpm"
download \
    "https://github.com/tarantool/tt/releases/download/v${TT_VERSION}/tt-${TT_VERSION}-linux-amd64.tar.gz" \
    "$CACHE_DIR/tt-${TT_VERSION}-linux-amd64.tar.gz"

for distro in "$@"; do
    image="$(image_for "$distro")"
    docker pull "$image" >/dev/null
    build_for "$distro" "$image"
    test_for \
        "$distro" "$image" "$(package_kind_for "$distro")" \
        '/target/x86_64-unknown-linux-gnu/release/libtarantool_wasm_rs.so'
done

echo 'Distribution matrix: PASS'
