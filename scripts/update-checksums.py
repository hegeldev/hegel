#!/usr/bin/env python3
"""Regenerate the baked-in libhegel SHA-256 checksums in lib/ffi/loader.ml.

Reads the targeted libhegel version from lib/ffi/loader.ml, lists the
``libhegel-<os>-<arch>.{so,dylib,dll}`` assets of the matching libhegel
GitHub release (via the ``gh`` CLI), downloads each one, computes its SHA-256
locally (we don't trust the published .sha256 sidecars — we recompute), and
rewrites the ``let checksums = [ ... ]`` table.

Usage: bump ``let version`` in lib/ffi/loader.ml, then run this script and
commit the diff. Requires the GitHub CLI (`gh`) on PATH.
"""

import hashlib
import re
import subprocess
import sys
import urllib.request
from pathlib import Path

LOADER = Path(__file__).resolve().parent.parent / "lib" / "ffi" / "loader.ml"
MONOREPO_SINCE = (0, 47, 2)

VERSION_RE = re.compile(r'let version = "([^"]+)"')
# Client-loadable artifacts only; the ".sha256" sidecars are excluded.
ASSET_RE = re.compile(r"^libhegel-([a-z0-9]+-[a-z0-9]+)\.(?:so|dylib|dll)$")
CHECKSUMS_RE = re.compile(r"let checksums =\n  \[.*?\n  \]\n;;", re.DOTALL)


def release_source(version: str) -> tuple[str, str]:
    """The (repo, tag) of the GitHub release libhegel ``version`` hangs off.

    libhegel releases live in the hegeldev/hegel monorepo since 0.47.2, tagged
    ``libhegel/<version>``; earlier releases stay on hegeldev/hegel-rust under
    ``libhegel-v<version>`` tags. Keep in sync with ``release_base`` in
    lib/ffi/loader.ml and .github/scripts/libhegel-release-url.sh.
    """
    if tuple(int(p) for p in version.split(".")) < MONOREPO_SINCE:
        return "hegeldev/hegel-rust", f"libhegel-v{version}"
    return "hegeldev/hegel", f"libhegel/{version}"


def main() -> int:
    src = LOADER.read_text(encoding="utf-8")
    version_match = VERSION_RE.search(src)
    if not version_match:
        sys.exit(f'could not find `let version = "..."` in {LOADER}')
    version = version_match.group(1)
    repo, tag = release_source(version)
    print(f"libhegel version: {version} ({repo} {tag})")

    names = subprocess.run(
        ["gh", "release", "view", tag, "--repo", repo,
         "--json", "assets", "--jq", ".assets[].name"],
        check=True, capture_output=True, text=True,
    ).stdout.split()

    keyed = {}
    for name in names:
        m = ASSET_RE.match(name)
        if m:
            keyed[m.group(1)] = name
    if not keyed:
        sys.exit(f"no libhegel-<os>-<arch> assets found in {repo} {tag}")

    base = f"https://github.com/{repo}/releases/download/{tag}"
    rows = []
    for key in sorted(keyed):
        print(f"  hashing {keyed[key]} ...")
        with urllib.request.urlopen(f"{base}/{keyed[key]}") as resp:
            rows.append((key, hashlib.sha256(resp.read()).hexdigest()))

    body = "\n".join(
        f'  {"[" if i == 0 else ";"} "{key}", "{digest}"'
        for i, (key, digest) in enumerate(rows)
    )
    block = f"let checksums =\n{body}\n  ]\n;;"
    new_src, n = CHECKSUMS_RE.subn(block, src)
    if n != 1:
        sys.exit("could not locate the `let checksums = [ ... ]` block to replace")

    LOADER.write_text(new_src, encoding="utf-8")
    print(f"updated {len(rows)} checksums in {LOADER}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
