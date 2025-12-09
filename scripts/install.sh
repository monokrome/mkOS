#!/bin/sh
# mkOS Network Installer
# Usage: curl -sL https://mkos.cc/install | sh
#        curl -sL https://mkos.cc/install | sh -s -- manifest.yaml
#        curl -sL https://mkos.cc/install | sh -s -- https://example.com/manifest.yaml
#        curl -sL https://mkos.cc/install | sh -s -- config.tar.gz
set -e

MKOS_RELEASE_URL="${MKOS_RELEASE_URL:-https://github.com/monokrome/mkos/releases/latest/download}"
INSTALLER_BIN="mkos-install"
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

warn() {
    printf '\033[1;33mWarning:\033[0m %s\n' "$1"
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

download_installer() {
    local arch="$1"
    local url="${MKOS_RELEASE_URL}/${INSTALLER_BIN}-${arch}"

    info "Downloading mkos-install for ${arch}..."

    if ! curl -fsSL "$url" -o "${TMP_DIR}/${INSTALLER_BIN}"; then
        die "Failed to download installer from ${url}"
    fi

    chmod +x "${TMP_DIR}/${INSTALLER_BIN}"
}

run_installer() {
    local manifest="$1"
    local installer="${TMP_DIR}/${INSTALLER_BIN}"

    # When this script is piped (curl ... | sh), stdin is consumed by curl
    # We need to redirect stdin from the terminal for interactive prompts
    if [ -n "$manifest" ]; then
        info "Running installer with manifest: ${manifest}"
        exec "$installer" "$manifest" </dev/tty
    else
        info "Running installer in interactive mode"
        exec "$installer" </dev/tty
    fi
}

main() {
    local manifest=""

    # Parse arguments
    while [ $# -gt 0 ]; do
        case "$1" in
            -h|--help)
                cat <<EOF
mkOS Network Installer

Usage:
    curl -sL https://mkos.cc/install | sh
    curl -sL https://mkos.cc/install | sh -s -- [OPTIONS] [MANIFEST]

Arguments:
    MANIFEST    Path, URL, or - for stdin. Supports YAML, JSON, or tar.gz

Options:
    -h, --help    Show this help message

Examples:
    # Interactive install
    curl -sL https://mkos.cc/install | sh

    # Install with local manifest
    curl -sL https://mkos.cc/install | sh -s -- /path/to/manifest.yaml

    # Install with remote manifest
    curl -sL https://mkos.cc/install | sh -s -- https://example.com/config.yaml

    # Install with manifest bundle (tar.gz with manifest.yaml and files)
    curl -sL https://mkos.cc/install | sh -s -- https://example.com/config.tar.gz

    # Pipe manifest from stdin
    cat manifest.yaml | curl -sL https://mkos.cc/install | sh -s -- -

Environment:
    MKOS_RELEASE_URL    Override the release download URL
EOF
                exit 0
                ;;
            -*)
                if [ "$1" = "-" ]; then
                    manifest="-"
                else
                    die "Unknown option: $1"
                fi
                shift
                ;;
            *)
                manifest="$1"
                shift
                ;;
        esac
    done

    check_root
    check_deps

    local arch
    arch="$(detect_arch)"

    TMP_DIR="$(mktemp -d)"

    download_installer "$arch"
    run_installer "$manifest"
}

main "$@"
