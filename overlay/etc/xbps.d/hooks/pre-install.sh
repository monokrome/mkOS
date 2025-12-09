#!/bin/bash
# Pre-install hook: create btrfs snapshot before updates
# xbps runs hooks in /etc/xbps.d/hooks/

if [[ -x /usr/local/bin/mkos-snapshot ]]; then
    /usr/local/bin/mkos-snapshot create "pre-update-$(date +%Y%m%d-%H%M%S)"
fi
