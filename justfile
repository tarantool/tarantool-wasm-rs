cargo_build_debug := "cargo build"
cargo_build_release := "cargo build --release --target x86_64-unknown-linux-gnu"

wkg_fetch := "wkg wit fetch"
wkg_build := "wkg wit build"
add_registry_patch := "./scripts/add_tarantool_registry.sh"

git_submodule_update_wasi_tls := "git submodule update --init --recursive third-party/wasi-tls"
git_submodule_update_tarantool := "git submodule update --init --recursive third-party/tarantool"

build_tarantool_with_wasm := "./scripts/build_tarantool_wasm.sh"

all:
    just submodules patch-wkg-config wit-fetch
    sh -c 'if [ "${RELEASE:-}" = "1" ]; then just build-release; else just build; fi'

submodules:
    just submodule-wasi-tls submodule-tarantool

submodule-wasi-tls:
    sh -c "{{git_submodule_update_wasi_tls}}"

submodule-tarantool:
    sh -c "{{git_submodule_update_tarantool}}"

build-tarantool:
    sh -c "{{build_tarantool_with_wasm}}"

install-wkg:
    sh -c "if ! command -v wkg > /dev/null; then cargo install wkg; fi"

patch-wkg-config:
    sh -c "{{add_registry_patch}}"

wit-fetch:
    sh -c "{{wkg_fetch}}"

wit-build:
    sh -c "{{wkg_build}}"

build:
    sh -c "{{cargo_build_debug}}"

build-release:
    sh -c "{{cargo_build_release}}"

install-luatest:
    if ! command -v luatest > /dev/null; then \
        tt rocks install luatest; \
    else \
        echo "luatest already installed"; \
    fi

test:
    PATH="$PWD/.rocks/bin:$PATH" luatest -v -l luatest.log
