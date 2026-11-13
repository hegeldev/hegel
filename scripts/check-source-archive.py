#!/usr/bin/env python3
"""Build a frontend from its release source archive.

    scripts/check-source-archive.py <frontend>

Builds the archive a release of `frontends/<frontend>` would attach to its
GitHub release (see `build_source_archive` in .github/scripts/release.py),
from the commit checked out, unpacks it outside the repository and builds
the frontend there with nothing but what the archive holds. Uncommitted
changes are not in the archive.

A frontend's build fetches its own dependencies (crates, npm packages, Maven
artifacts) as it does in a checkout; the toolchains must be installed.
"""

from __future__ import annotations

import os
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / ".github" / "scripts"))

import release  # noqa: E402

VERSION = "0.0.0"

# The commands that build each frontend, run in its directory of the unpacked
# archive. `{archive}` and `{sha256}` are the archive file and its checksum,
# `{checkout}` this repository and `{scratch}` an empty directory.
BUILDS: dict[str, list[list[str]]] = {
    "rust": [["cargo", "build", "--locked"]],
    "python": [["uv", "build", "--wheel", "--out-dir", "{scratch}"]],
    "typescript": [["npm", "ci"], ["just", "build"]],
    # As a consumer does: CMake's FetchContent, given the archive as its URL.
    "cpp": [
        [
            "cmake",
            "-S", "{checkout}/frontends/cpp/tests/consumer/archive",
            "-B", "{scratch}",
            "-DHEGEL_ARCHIVE={archive}",
            "-DHEGEL_ARCHIVE_SHA256={sha256}",
        ],
        ["cmake", "--build", "{scratch}"],
        ["{scratch}/consumer"],
    ],
    "java": [["just", "build"]],
    "csharp": [["just", "build"]],
    "ocaml": [["just", "build-libhegel"], ["opam", "exec", "--", "dune", "build", "@install"]],
    "go": [["go", "build", "./..."], ["go", "vet", "./..."]],
}


def main(frontend: str) -> None:
    component = next(c for c in release.FRONTENDS if c.path == f"frontends/{frontend}")
    with tempfile.TemporaryDirectory() as tmp:
        archive, checksum = release.build_source_archive(
            ROOT, component, VERSION, "HEAD", Path(tmp) / "archive"
        )
        with tarfile.open(archive) as tar:
            tar.extractall(Path(tmp) / "source")
        source = Path(tmp) / "source" / component.source_archive_stem(VERSION)
        lockfile = (source / "Cargo.lock").read_bytes()
        scratch = Path(tmp) / "scratch"
        scratch.mkdir()

        # The build must not find the checkout's engine in place of the
        # archive's.
        env = {
            k: v
            for k, v in os.environ.items()
            if k not in ("CARGO_TARGET_DIR", "HEGEL_LIBHEGEL_PATH", "HEGEL_C_LIB_DIR")
        }
        for command in BUILDS[frontend]:
            command = [
                arg.format(
                    archive=archive,
                    sha256=checksum.read_text().split()[0],
                    checkout=ROOT,
                    scratch=scratch,
                )
                for arg in command
            ]
            print("+", " ".join(command), flush=True)
            subprocess.run(command, check=True, cwd=source / component.path, env=env)

        if (source / "Cargo.lock").read_bytes() != lockfile:
            raise SystemExit(
                "the build rewrote the archive's Cargo.lock: prune_lockfile in "
                ".github/scripts/release.py no longer writes it as cargo does"
            )
    print(f"frontends/{frontend} builds from its source archive.")


if __name__ == "__main__":
    if len(sys.argv) != 2 or sys.argv[1] not in BUILDS:
        raise SystemExit(f"usage: {sys.argv[0]} <{'|'.join(BUILDS)}>")
    main(sys.argv[1])
