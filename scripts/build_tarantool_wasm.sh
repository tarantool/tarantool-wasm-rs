#!/usr/bin/env bash
# Build Tarantool with WASM module support.

set -euo pipefail

# Script directory
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# Project root
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

TT_DIR="${PROJECT_DIR}/third-party/tarantool"
BUILD_DIR="${TT_DIR}/build"
PATCH_FILE="${PROJECT_DIR}/patches/tarantool-wasm.patch"

# Portable CPU count: Linux (nproc) / macOS (*BSD sysctl) / fallback 1
if command -v nproc >/dev/null 2>&1; then
  JOBS="$(nproc)"
elif command -v sysctl >/dev/null 2>&1; then
  JOBS="$(sysctl -n hw.ncpu || echo 1)"
else
  JOBS=1
fi
echo "[INFO] Found proc: ${JOBS}"

cd "${TT_DIR}"

# If patch file exists, reset tree and apply it
if [[ -f "${PATCH_FILE}" ]]; then
  echo "[INFO] Found patch: ${PATCH_FILE}"
  echo "[INFO] Cleaning repo and resetting to HEAD..."
  git reset --hard HEAD
  git clean -fdx
  echo "[INFO] Applying patch..."
  patch -p1 < "${PATCH_FILE}"
fi

# Clean build directory
rm -rf -- "${BUILD_DIR}"

# Configure
cmake -S . -B "${BUILD_DIR}" \
  -DCMAKE_BUILD_TYPE=RelWithDebInfo \
  -DENABLE_BACKTRACE=ON \
  -DENABLE_WERROR=ON \
  -DTARANTOOL_WASM=ON

# Build
cmake --build "${BUILD_DIR}" -j"${JOBS}"

# Install (may prompt for sudo password)
sudo cmake --install "${BUILD_DIR}"
