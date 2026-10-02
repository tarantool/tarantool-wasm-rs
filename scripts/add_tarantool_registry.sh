#!/bin/bash

set -euo pipefail

CONFIG_FILE="${WKG_CONFIG_FILE:-$HOME/.config/wasm-pkg/config.toml}"
TOML_KEY='"tarantool:tarantool" = { registry = "ghcr.io", metadata = { preferredProtocol = "oci", oci = { registry = "ghcr.io", namespacePrefix = "mandesero/" } } }'

mkdir -p "$(dirname "$CONFIG_FILE")"

# Initialize the fallback registry before checking the Tarantool override.
# Existing default and namespace-specific registries take precedence.
# Older wkg reports missing defaults on stdout; newer versions use stderr
# and prefix the message with "warning:".
registry_status=$(wkg config --config "$CONFIG_FILE" 2>&1) || {
    printf '%s\n' "$registry_status" >&2
    exit 1
}
if [[ ! -f "$CONFIG_FILE" || "$registry_status" == *'Default registry is not set'* ]]; then
    wkg config --config "$CONFIG_FILE" --default-registry wasi.dev
fi

# Skip if already present
if grep -q '"tarantool:tarantool"' "$CONFIG_FILE" 2>/dev/null; then
    echo "tarantool:tarantool override already exists. Skipping."
    exit 0
fi

# Use a temporary file rather than GNU-only sed -i.
temp_config=$(mktemp "${CONFIG_FILE}.XXXXXX")
trap 'rm -f "$temp_config"' EXIT
cp -p "$CONFIG_FILE" "$temp_config"
awk -v key="$TOML_KEY" '
    /^[[:space:]]*\[package_registry_overrides\][[:space:]]*(#.*)?$/ {
        print
        print key
        inserted = 1
        next
    }
    { print }
    END {
        if (!inserted) {
            print ""
            print "[package_registry_overrides]"
            print key
        }
    }
' "$CONFIG_FILE" > "$temp_config"
wkg config --config "$temp_config" > /dev/null
mv "$temp_config" "$CONFIG_FILE"

echo "Done."
