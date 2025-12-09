#!/bin/bash
set -euo pipefail

# Build dwl from monokrome's config
# Usage: ./build-dwl.sh [output-dir]

DWL_CONFIG_REPO="https://github.com/monokrome/dwl-config.git"
BUILD_DIR="${1:-/tmp/dwl-build}"
OUTPUT_DIR="${2:-$PWD/bin}"

echo "==> Cloning dwl-config..."
rm -rf "$BUILD_DIR"
git clone --recursive "$DWL_CONFIG_REPO" "$BUILD_DIR"

echo "==> Building dwl..."
cd "$BUILD_DIR"
make

echo "==> Copying binary..."
mkdir -p "$OUTPUT_DIR"
cp bin/dwl "$OUTPUT_DIR/dwl"

echo "==> Done: $OUTPUT_DIR/dwl"
