# Define paths and tools
cargo_build := "cargo build"
wkg_fetch := "wkg wit fetch"
wkg_build := "wkg wit build"
git_submodule_update := "git submodule update --init --recursive"
add_registry_patch := "./add_tarantool_registry.sh"
link_shared_lib := "ln -sf target/debug/libtarantool_wasm_rs.so my_module.so"

# Default task to run when calling `just`
all:
    echo "Checking if wkg is installed..."
    sh -c "if ! command -v wkg > /dev/null; then cargo install wkg; fi"
    echo "Updating submodules, patching wkg config, fetching and building WIT dependencies, building Rust project, and creating symlink..."
    sh -c "{{git_submodule_update}}"
    sh -c "{{add_registry_patch}}"
    sh -c "{{wkg_fetch}}"
    sh -c "{{wkg_build}}"
    sh -c "{{cargo_build}}"
    sh -c "{{link_shared_lib}}"

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

# Task to create symlink for the shared library
link:
    sh -c "{{link_shared_lib}}"
