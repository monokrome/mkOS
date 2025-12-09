#!/usr/bin/env python3
"""
Simple caching proxy for pacman mirrors.
Caches packages locally to avoid hitting upstream on repeated installs.

Usage:
    ./mirror-cache.py [--port 8080] [--upstream https://mirrors.xmission.com/artix]

In VM mirrorlist:
    Server = http://10.0.2.2:8080/$repo/os/$arch
"""

import argparse
import hashlib
import os
import sys
import time
from http.server import HTTPServer, BaseHTTPRequestHandler
from pathlib import Path
from urllib.request import urlopen, Request
from urllib.error import HTTPError, URLError

CACHE_DIR = Path("/tmp/pacman-cache")


class CachingProxyHandler(BaseHTTPRequestHandler):
    upstream = "https://mirrors.xmission.com/artix"

    def do_GET(self):
        # Build cache key from path
        cache_key = hashlib.md5(self.path.encode()).hexdigest()
        cache_file = CACHE_DIR / cache_key
        meta_file = CACHE_DIR / f"{cache_key}.meta"

        # Check cache
        if cache_file.exists() and meta_file.exists():
            meta = meta_file.read_text().split("\n")
            content_type = meta[0] if meta else "application/octet-stream"

            self.send_response(200)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", cache_file.stat().st_size)
            self.send_header("X-Cache", "HIT")
            self.end_headers()

            with open(cache_file, "rb") as f:
                self.wfile.write(f.read())

            self.log_message("CACHE HIT: %s", self.path)
            return

        # Fetch from upstream
        upstream_url = f"{self.upstream}{self.path}"
        try:
            req = Request(upstream_url, headers={"User-Agent": "mkos-mirror-cache/1.0"})
            with urlopen(req, timeout=30) as resp:
                content = resp.read()
                content_type = resp.headers.get("Content-Type", "application/octet-stream")

                # Cache the response (only cache .pkg.tar files and db files)
                if self.path.endswith((".pkg.tar.zst", ".pkg.tar.xz", ".db", ".db.sig", ".files")):
                    CACHE_DIR.mkdir(parents=True, exist_ok=True)
                    cache_file.write_bytes(content)
                    meta_file.write_text(f"{content_type}\n{upstream_url}")
                    self.log_message("CACHED: %s (%d bytes)", self.path, len(content))

                self.send_response(200)
                self.send_header("Content-Type", content_type)
                self.send_header("Content-Length", len(content))
                self.send_header("X-Cache", "MISS")
                self.end_headers()
                self.wfile.write(content)

        except HTTPError as e:
            self.send_error(e.code, str(e.reason))
        except URLError as e:
            self.send_error(502, f"Upstream error: {e.reason}")
        except Exception as e:
            self.send_error(500, str(e))

    def do_HEAD(self):
        # Build cache key from path
        cache_key = hashlib.md5(self.path.encode()).hexdigest()
        cache_file = CACHE_DIR / cache_key
        meta_file = CACHE_DIR / f"{cache_key}.meta"

        if cache_file.exists() and meta_file.exists():
            meta = meta_file.read_text().split("\n")
            content_type = meta[0] if meta else "application/octet-stream"
            self.send_response(200)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", cache_file.stat().st_size)
            self.send_header("X-Cache", "HIT")
            self.end_headers()
            return

        # Check upstream
        upstream_url = f"{self.upstream}{self.path}"
        try:
            req = Request(upstream_url, method="HEAD", headers={"User-Agent": "mkos-mirror-cache/1.0"})
            with urlopen(req, timeout=10) as resp:
                self.send_response(200)
                self.send_header("Content-Type", resp.headers.get("Content-Type", "application/octet-stream"))
                if resp.headers.get("Content-Length"):
                    self.send_header("Content-Length", resp.headers["Content-Length"])
                self.send_header("X-Cache", "MISS")
                self.end_headers()
        except HTTPError as e:
            self.send_error(e.code, str(e.reason))
        except URLError as e:
            self.send_error(502, f"Upstream error: {e.reason}")

    def log_message(self, format, *args):
        print(f"[{time.strftime('%H:%M:%S')}] {format % args}")


def main():
    parser = argparse.ArgumentParser(description="Caching proxy for pacman mirrors")
    parser.add_argument("--port", type=int, default=8080, help="Port to listen on")
    parser.add_argument("--upstream", default="https://mirror.clarkson.edu/artix-linux/repos",
                        help="Upstream mirror URL")
    args = parser.parse_args()

    CachingProxyHandler.upstream = args.upstream.rstrip("/")

    CACHE_DIR.mkdir(parents=True, exist_ok=True)

    server = HTTPServer(("0.0.0.0", args.port), CachingProxyHandler)
    print(f"Mirror cache running on http://localhost:{args.port}")
    print(f"Upstream: {args.upstream}")
    print(f"Cache dir: {CACHE_DIR}")
    print(f"\nIn VM mirrorlist use:")
    print(f"  Server = http://10.0.2.2:{args.port}/$repo/os/$arch")
    print()

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down...")
        server.shutdown()


if __name__ == "__main__":
    main()
