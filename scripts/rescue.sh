#!/bin/sh
# mkOS Rescue Tool
# Usage: curl -sL https://mkos.cc/rescue | sh
#        curl -sL https://mkos.cc/rescue | sh -s -- /dev/sda1 /dev/sda2
set -e

MKOS_RELEASE_URL="${MKOS_RELEASE_URL:-https://github.com/monokrome/mkos/releases/latest/download}"
RESCUE_BIN="mkos-rescue"
TMP_DIR=""

cleanup() {
    if [ -n "$TMP_DIR" ] && [ -d "$TMP_DIR" ]; then
        rm -rf "$TMP_DIR"
    fi
}

trap cleanup EXIT

die() {
    printf '\033[1;31mError:\033[0m %s\n' "$1" >&2
    exit 1
}

info() {
    printf '\033[1;34m==>\033[0m %s\n' "$1"
}

check_root() {
    if [ "$(id -u)" -ne 0 ]; then
        die "This script must be run as root"
    fi
}

detect_arch() {
    case "$(uname -m)" in
        x86_64) echo "x86_64" ;;
        aarch64) echo "aarch64" ;;
        *) die "Unsupported architecture: $(uname -m)" ;;
    esac
}

check_deps() {
    for cmd in curl; do
        if ! command -v "$cmd" >/dev/null 2>&1; then
            die "Required command not found: $cmd"
        fi
    done
}

download_rescue() {
    local arch="$1"
    local url="${MKOS_RELEASE_URL}/${RESCUE_BIN}-${arch}"

    info "Downloading mkos-rescue for ${arch}..."

    if ! curl -fsSL "$url" -o "${TMP_DIR}/${RESCUE_BIN}"; then
        die "Failed to download rescue tool from ${url}"
    fi

    chmod +x "${TMP_DIR}/${RESCUE_BIN}"
}

run_rescue() {
    local efi_part="$1"
    local luks_part="$2"
    local rescue="${TMP_DIR}/${RESCUE_BIN}"

    if [ -n "$efi_part" ] && [ -n "$luks_part" ]; then
        info "Running rescue with partitions: EFI=${efi_part} LUKS=${luks_part}"
        exec "$rescue" "$efi_part" "$luks_part" </dev/tty
    else
        info "Running rescue in auto-detect mode"
        exec "$rescue" </dev/tty
    fi
}

main() {
    local efi_part=""
    local luks_part=""

    while [ $# -gt 0 ]; do
        case "$1" in
            -h|--help)
                cat <<EOF
mkOS Rescue Tool

Usage:
    curl -sL https://mkos.cc/rescue | sh
    curl -sL https://mkos.cc/rescue | sh -s -- [EFI_PARTITION LUKS_PARTITION]

Arguments:
    EFI_PARTITION     EFI system partition (e.g., /dev/sda1)
    LUKS_PARTITION    LUKS-encrypted system partition (e.g., /dev/sda2)

    If no arguments are given, partitions are auto-detected.

Options:
    -h, --help    Show this help message

Examples:
    # Auto-detect partitions
    curl -sL https://mkos.cc/rescue | sh

    # Specify partitions explicitly
    curl -sL https://mkos.cc/rescue | sh -s -- /dev/nvme0n1p1 /dev/nvme0n1p2

Environment:
    MKOS_RELEASE_URL    Override the release download URL
EOF
                exit 0
                ;;
            -*)
                die "Unknown option: $1"
                ;;
            *)
                if [ -z "$efi_part" ]; then
                    efi_part="$1"
                elif [ -z "$luks_part" ]; then
                    luks_part="$1"
                else
                    die "Too many arguments. Expected at most 2 (EFI_PARTITION LUKS_PARTITION)"
                fi
                shift
                ;;
        esac
    done

    if [ -n "$efi_part" ] && [ -z "$luks_part" ]; then
        die "Both EFI_PARTITION and LUKS_PARTITION must be provided, or neither for auto-detection"
    fi

    check_root
    check_deps

    local arch
    arch="$(detect_arch)"

    TMP_DIR="$(mktemp -d)"

    download_rescue "$arch"
    run_rescue "$efi_part" "$luks_part"
}

main "$@"
