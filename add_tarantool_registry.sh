#!/bin/bash

CONFIG_FILE="${WKG_CONFIG_FILE:-$HOME/.config/wasm-pkg/config.toml}"
TOML_KEY='"tarantool:tarantool" = { registry = "ghcr.io", metadata = { preferredProtocol = "oci", oci = { registry = "ghcr.io", namespacePrefix = "mandesero/" } } }'

mkdir -p "$(dirname "$CONFIG_FILE")"

# Skip if already present
if grep -q '"tarantool:tarantool"' "$CONFIG_FILE" 2>/dev/null; then
    echo "tarantool:tarantool override already exists. Skipping."
    exit 0
fi

if grep -q '^\[package_registry_overrides\]' "$CONFIG_FILE" 2>/dev/null; then
    echo "Appending inside existing [package_registry_overrides] section..."

    line_num=$(grep -n '^\[package_registry_overrides\]' "$CONFIG_FILE" | cut -d: -f1)

    next_section_line=$(awk "NR>$line_num && /^\[.*\]/ {print NR; exit}" "$CONFIG_FILE")

    insert_line=$((line_num + 1))

    if [ -z "$next_section_line" ]; then
        # No next section - check if line after header is empty
        sed -i "${insert_line}s/^/${TOML_KEY}\n/" "$CONFIG_FILE"
    else
        # Check if the line after header is blank
        line_after_header=$(sed -n "${insert_line}p" "$CONFIG_FILE")
        if [[ -z "$line_after_header" ]]; then
            # Replace the blank line
            sed -i "${insert_line}s|^|$TOML_KEY\n|" "$CONFIG_FILE"
        else
            # Insert directly after header
            sed -i "${insert_line}i $TOML_KEY" "$CONFIG_FILE"
        fi
    fi
else
    echo "Creating new [package_registry_overrides] section..."

    {
        echo "[package_registry_overrides]"
        echo "$TOML_KEY"
    } >> "$CONFIG_FILE"
fi

echo "Done."
