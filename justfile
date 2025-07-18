# Define paths and tools
cargo_build := "cargo build"
wkg_fetch := "wkg wit fetch"
wkg_build := "wkg wit build"
git_submodule_update := "git submodule update --init --recursive"
add_registry_patch := "./scripts/add_tarantool_registry.sh"
build_tarantool_with_wasm := "./scripts/build_tarantool_wasm.sh"
custom_tarantool_bin := "third-party/tarantool/build/src"

# Default task to run when calling `just`
all:
    echo "Checking if wkg is installed..."
    sh -c "if ! command -v wkg > /dev/null; then cargo install wkg; fi"
    echo "Updating submodules, patching wkg config, fetching and building WIT dependencies, building Rust project..."
    sh -c "{{git_submodule_update}}"
    sh -c "{{add_registry_patch}}"
    sh -c "{{wkg_fetch}}"
    sh -c "{{wkg_build}}"
    sh -c "{{cargo_build}}"

# Task to update submodules
submodules:
    sh -c "{{git_submodule_update}}"

# Task to patch wkg config
patch-wkg-config:
    sh -c "{{add_registry_patch}}"

# Task to fetch dependencies with wkg
fetch:
    sh -c "{{wkg_fetch}}"

# Task to build WIT dependencies
wit-build:
    sh -c "{{wkg_build}}"

# Task to build the Rust project
build:
    sh -c "{{cargo_build}}"

# Build and install Tarantool with WASM module support
build-tarantool:
    sh -c {{build_tarantool_with_wasm}}

# One-time setup: install luatest using custom tarantool
install-luatest:
    if ! command -v luatest > /dev/null; then \
        tt rocks install luatest; \
    else \
        echo "luatest already installed"; \
    fi

# Run tests with proper PATH for tarantool and luatest
test:
    PATH="$PWD/.rocks/bin:$PATH" luatest -v -l luatest.log
