set positional-arguments

# Keep wkg and the registry setup script on the same config path on both OSes.
export WKG_CONFIG_FILE := env("WKG_CONFIG_FILE", env("HOME") / ".config/wasm-pkg/config.toml")

wkg_fetch := "wkg wit fetch --type wasm"
wkg_build := "wkg wit build"
add_registry_patch := "./scripts/add_tarantool_registry.sh"

csharp_image := env("TWR_CSHARP_IMAGE", "tarawasm:csharp-sdk")
csharp_run := 'docker run --rm --pull=never --platform linux/amd64 -v "' + justfile_directory() + ':/work" -w /work/guests/csharp "' + csharp_image + '"'
csharp_bindgen := "/opt/tarawasm-csharp/packages/bytecodealliance.componentize.dotnet.witbindgen/0.8.0-preview00011/tools/linux-x64/wit-bindgen"

git_submodule_update_wasi_tls := "git submodule update --init --recursive --jobs 8 third-party/wasi-tls"

all: submodule-wasi-tls patch-wkg-config wit-fetch
    sh -c 'if [ "${RELEASE:-}" = "1" ]; then just build-release; else just build; fi'

submodule-wasi-tls:
    sh -c "{{ git_submodule_update_wasi_tls }}"

install-wkg:
    sh -c "if ! command -v wkg > /dev/null; then cargo install wkg --locked; fi"

patch-wkg-config:
    sh -c "{{ add_registry_patch }}"

wit-fetch:
    sh -c "{{ wkg_fetch }}"

wit-build:
    sh -c "{{ wkg_build }}"

build: && (install-module "debug")
    cargo build

build-release: && (install-module "release")
    cargo build --release

# Language SDK artifacts and their separately maintained compatibility suite.
python-sdk:
    bash guests/python/build-artifacts.sh

rust-sdk: patch-wkg-config
    rm -rf guests/rust/wit/deps
    cd guests/rust && {{ wkg_fetch }}
    cargo build --locked --manifest-path guests/rust/Cargo.toml
    cargo package --locked --allow-dirty --manifest-path guests/rust/Cargo.toml

test-python:
    python3 -m unittest discover -s guests/python/tests/unit -p '*_test.py'
    cargo test -p twr-python-ssl-host --lib
    bash guests/python/test.sh

csharp-sdk: patch-wkg-config
    rm -rf guests/csharp/wit/deps guests/csharp/src/obj/bindings
    cd guests/csharp && {{ wkg_fetch }}
    {{ csharp_run }} {{ csharp_bindgen }} c-sharp wit --world tarantool-sdk --runtime native-aot --with-wit-results --internal --out-dir src/obj/bindings
    {{ csharp_run }} dotnet pack src/Tarantool.Wasm.csproj -c Release -o dist

# Install the native library built for the selected Cargo profile.
install-module profile="debug":
    #!/usr/bin/env sh
    set -eu
    case "$1" in
        debug|release) profile="$1" ;;
        *) echo "Unsupported profile: $1" >&2; exit 1 ;;
    esac
    case "$(uname -s)" in
        Darwin) extension=dylib ;;
        Linux) extension=so ;;
        *) echo "Unsupported platform: $(uname -s)" >&2; exit 1 ;;
    esac
    target_dir="${CARGO_TARGET_DIR:-target}"
    if [ -n "${CARGO_BUILD_TARGET:-}" ]; then
        target_dir="$target_dir/$CARGO_BUILD_TARGET"
    fi
    mkdir -p .rocks/lib/tarantool
    # Replace the inode atomically: overwriting a mapped dylib can leave macOS
    # with cached code-signing pages and kill the next loader with SIGKILL.
    module_tmp=".rocks/lib/tarantool/.wasm-$profile.$$.$extension"
    trap 'rm -f "$module_tmp"' 0
    cp "$target_dir/$profile/libtarantool_wasm_rs.$extension" "$module_tmp"
    mv -f "$module_tmp" ".rocks/lib/tarantool/wasm.$extension"

install-luatest:
    if ! command -v luatest > /dev/null; then \
        tt rocks install luatest; \
    else \
        echo "luatest already installed"; \
    fi

test *args:
    PATH="$PWD/.rocks/bin:$PATH" luatest -v -l luatest.log "$@"
