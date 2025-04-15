# Define paths and tools
cargo_build := "cargo build"
wkg_fetch := "wkg wit fetch"
git_submodule_update := "git submodule update --init --recursive"

# Default task to run when calling `just`
all:
    echo "Checking if wkg is installed..."
    sh -c "if ! command -v wkg > /dev/null; then cargo install wkg; fi"
    echo "Updating submodules, fetching dependencies, and building the project..."
    sh -c "{{git_submodule_update}}"
    sh -c "{{wkg_fetch}}"
    sh -c "{{cargo_build}}"

# Task to update submodules
submodules:
    sh -c "{{git_submodule_update}}"

# Task to fetch dependencies with wkg
fetch:
    sh -c "{{wkg_fetch}}"

# Task to build the Rust project
build:
    sh -c "{{cargo_build}}"
