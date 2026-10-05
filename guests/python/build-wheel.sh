#!/bin/sh
set -eu
sdk_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
python=${PYTHON:-python3}
output=${1:-"$sdk_dir/dist"}
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-946684800}"
tools=$(mktemp -d "${TMPDIR:-/tmp}/twr-sdk-packaging.XXXXXX")
trap 'rm -rf -- "$tools"' EXIT HUP INT TERM
# Explicit temporary backend paths also work with the Debian pip bundled in
# tarawasm:v0.3.0, whose isolated builds otherwise select system setuptools.
"$python" -m pip install --disable-pip-version-check --target "$tools" \
    setuptools==80.9.0 wheel==0.45.1
PYTHONPATH="$tools" "$python" -m pip wheel --disable-pip-version-check \
    --no-build-isolation --no-deps -w "$output" "$sdk_dir"
