#!/bin/bash
set -euo pipefail

# Build mkOS live ISO
# Requires: void-mklive

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
OUTPUT_DIR="${1:-$PROJECT_DIR/out}"

# Check for void-mklive
if ! command -v mklive.sh &>/dev/null; then
    echo "Error: void-mklive not found"
    echo "Clone it: git clone https://github.com/void-linux/void-mklive.git"
    exit 1
fi

# Build the installer binary
echo "==> Building installer..."
cd "$PROJECT_DIR/installer"
cargo build --release
INSTALLER_BIN="$PROJECT_DIR/installer/target/release/mkos-installer"

# Build dwl
echo "==> Building dwl..."
"$SCRIPT_DIR/build-dwl.sh" /tmp/dwl-build "$PROJECT_DIR/out"

# Collect packages
BASE_PACKAGES=$(grep -v '^#' "$PROJECT_DIR/packages/base.txt" | tr '\n' ' ')
DESKTOP_PACKAGES=$(grep -v '^#' "$PROJECT_DIR/packages/desktop.txt" | tr '\n' ' ')

# Create ISO
echo "==> Building ISO..."
mkdir -p "$OUTPUT_DIR"

mklive.sh \
    -a x86_64 \
    -r https://repo-default.voidlinux.org/current \
    -p "$BASE_PACKAGES $DESKTOP_PACKAGES" \
    -I "$INSTALLER_BIN:/usr/local/bin/mkos-installer" \
    -I "$PROJECT_DIR/out/dwl:/usr/local/bin/dwl" \
    -I "$PROJECT_DIR/overlay/etc/greetd/config.toml:/etc/greetd/config.toml" \
    -I "$PROJECT_DIR/overlay/usr/share/wayland-sessions/dwl.desktop:/usr/share/wayland-sessions/dwl.desktop" \
    -o "$OUTPUT_DIR/mkos-live.iso"

echo "==> Done: $OUTPUT_DIR/mkos-live.iso"
