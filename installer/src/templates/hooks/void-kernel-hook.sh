#!/bin/sh
# mkOS kernel post-install hook for Void Linux
# Called by xbps when a kernel package is installed/upgraded
# Arguments: $1 = kernel version, $2 = kernel package name

VERSION="$1"

if [ -z "$VERSION" ]; then
    echo "ERROR: No kernel version provided"
    exit 1
fi

echo "==> mkOS: Rebuilding UKI for kernel $VERSION..."
/usr/local/bin/mkos-rebuild-uki

exit 0
