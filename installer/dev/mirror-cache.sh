#!/bin/bash
# Local pacman mirror cache for development
# Caches packages to avoid hammering upstream mirrors

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CACHE_DIR="/tmp/pacman-cache"

case "${1:-start}" in
    start)
        if pgrep -f "mirror-cache.py" > /dev/null; then
            echo "Mirror cache already running"
            exit 0
        fi
        "$SCRIPT_DIR/mirror-cache.py" &
        echo "Mirror cache started on http://localhost:8080"
        echo "In VM mirrorlist: Server = http://10.0.2.2:8080/\$repo/os/\$arch"
        ;;
    stop)
        pkill -f "mirror-cache.py"
        echo "Mirror cache stopped"
        ;;
    status)
        if pgrep -f "mirror-cache.py" > /dev/null; then
            echo "Running"
            du -sh "$CACHE_DIR" 2>/dev/null || echo "Cache: empty"
        else
            echo "Not running"
        fi
        ;;
    clear)
        rm -rf "$CACHE_DIR"/*
        echo "Cache cleared"
        ;;
    *)
        echo "Usage: $0 {start|stop|status|clear}"
        exit 1
        ;;
esac
