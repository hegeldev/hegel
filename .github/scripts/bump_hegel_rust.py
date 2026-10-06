"""Bump the pinned engine (the `hegeltest-c` crate) to a new release.

hegel-python builds libhegel from source: Cargo.toml pins `hegeltest-c` with
an exact `=X.Y.Z` requirement, and Cargo.lock records the resolved crate. This
script moves both to the requested version (or the latest on crates.io), writes
a RELEASE.md, and leaves a commit on a per-version branch. It does not push:
the workflow first realigns the bindings to the new engine, amends that into
the commit, and pushes once.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
RUST_REPO = "hegeldev/hegel-rust"
CRATE = "hegeltest-c"
# One branch per pinned version. A fixed, reused branch would be
# force-recreated from main on every release, clobbering any manual or agent
# work pushed to its open PR; a per-version branch keeps re-runs of the *same*
# version idempotent while never touching another version's work. The
# workflow closes superseded bot-only bump PRs after pushing.
BRANCH_PREFIX = "ci/bump-hegel-rust-"
CARGO_TOML = ROOT / "Cargo.toml"
CARGO_LOCK = ROOT / "Cargo.lock"
RELEASE_MD = ROOT / "RELEASE.md"
# Records which engine version its divergences were observed against.
ENGINE_ISSUES = ROOT / "docs" / "engine-issues.md"

PIN = re.compile(rf'^({re.escape(CRATE)} = \{{ version = "=)([^"]+)(")', re.MULTILINE)


def git(*args: str) -> None:
    subprocess.run(["git", *args], check=True, cwd=ROOT)


def set_output(name: str, value: str) -> None:
    """Expose a step output to later workflow steps (no-op outside Actions)."""
    out = os.environ.get("GITHUB_OUTPUT")
    if not out:
        return
    with open(out, "a") as f:
        f.write(f"{name}={value}\n")


def get_pinned_version() -> str:
    m = PIN.search(CARGO_TOML.read_text())
    assert m is not None, f"could not find an exact {CRATE} pin in Cargo.toml"
    return m.group(2)


def latest_version() -> str:
    request = urllib.request.Request(
        f"https://crates.io/api/v1/crates/{CRATE}",
        # crates.io rejects requests without a descriptive User-Agent.
        headers={"User-Agent": "hegel-python bump script (https://github.com/hegeldev/hegel-python)"},
    )
    with urllib.request.urlopen(request) as response:
        return json.load(response)["crate"]["max_stable_version"]


def pin(version: str) -> None:
    text, n = PIN.subn(rf"\g<1>{version}\g<3>", CARGO_TOML.read_text(), count=1)
    assert n == 1
    CARGO_TOML.write_text(text)
    # A just-published crate can take a little while to reach the index.
    for attempt in range(10):
        result = subprocess.run(
            ["cargo", "update", "-p", CRATE, "--precise", version], cwd=ROOT
        )
        if result.returncode == 0:
            return
        print(f"cargo update failed (attempt {attempt + 1}); retrying")
        time.sleep(30)
    raise RuntimeError(f"could not lock {CRATE} {version}")


def bump(requested: str) -> None:
    current = get_pinned_version()
    new = requested or latest_version()

    if new == current:
        print(f"Already pinned to {CRATE} {current}; nothing to do.")
        set_output("bumped", "false")
        return

    pin(new)
    if ENGINE_ISSUES.exists():
        ENGINE_ISSUES.write_text(
            ENGINE_ISSUES.read_text().replace(
                f"`{CRATE}` {current}", f"`{CRATE}` {new}"
            )
        )

    current_url = f"https://github.com/{RUST_REPO}/releases/tag/libhegel-v{current}"
    new_url = f"https://github.com/{RUST_REPO}/releases/tag/libhegel-v{new}"

    RELEASE_MD.write_text(
        "RELEASE_TYPE: patch\n\n"
        f"This patch bumps our pinned libhegel (`{CRATE}`, from "
        f"[hegel-rust](https://github.com/{RUST_REPO})) from "
        f"[{current}]({current_url}) to [{new}]({new_url}).\n"
    )

    app_id = os.environ["HEGEL_RELEASE_APP_ID"]
    git("config", "user.name", "hegel-release[bot]")
    git("config", "user.email", f"{app_id}+hegel-release[bot]@users.noreply.github.com")

    branch = BRANCH_PREFIX + new
    git("checkout", "-B", branch)
    paths = [CARGO_TOML, CARGO_LOCK, RELEASE_MD]
    if ENGINE_ISSUES.exists():
        paths.append(ENGINE_ISSUES)
    git("add", *map(str, paths))
    git(
        "commit",
        "-m",
        f"Bump pinned libhegel to {new}",
        "-m",
        f"Pins {CRATE} from {current} to {new} ({new_url}).",
    )

    set_output("bumped", "true")
    set_output("version", new)
    set_output("branch", branch)


if __name__ == "__main__":
    # An optional argument pins that exact version; with none we take the
    # latest on crates.io. The repository_dispatch trigger passes
    # client_payload.version, which is the hegel-c (`hegeltest-c`) version.
    bump(sys.argv[1] if len(sys.argv) > 1 else "")
