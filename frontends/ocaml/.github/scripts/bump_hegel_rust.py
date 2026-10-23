"""Pin a new libhegel release and open/update the bump PR.

Resolves the target libhegel release tag (an explicit `libhegel/<version>`
argument, else the latest libhegel release), writes its version into
`lib/ffi/loader.ml`, regenerates the baked-in checksums via the existing
`scripts/update-checksums.py`, drops a `RELEASE.md` so merging the PR cuts a
hegel-ocaml release, and force-pushes a fixed branch (updating an already-open
PR in place rather than stacking one PR per release).

libhegel is released from the hegeldev/hegel monorepo, where every component
is tagged `<path>/<version>`: a libhegel release is the GitHub release (with
the binaries attached) on a `libhegel/<version>` tag, so this script works in
terms of `libhegel/` tags throughout. Releases before 0.47.2 live on
hegeldev/hegel-rust under `libhegel-v<version>` tags; the currently pinned
version may still be one of those.

Requires the GitHub CLI (`gh`) on PATH with `GH_TOKEN` set.
"""

import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
LIBHEGEL_REPO = "hegeldev/hegel"
BRANCH = "ci/bump-hegel-rust"
LOADER = ROOT / "lib" / "ffi" / "loader.ml"
UPDATE_CHECKSUMS = ROOT / "scripts" / "update-checksums.py"
RELEASE_MD = ROOT / "RELEASE.md"

WORKFLOW_URL = (
    "https://github.com/hegeldev/hegel-ocaml/blob/main/.github/workflows/bump-hegel-rust.yml"
)

VERSION_RE = re.compile(r'^let version = "([^"]+)"', re.MULTILINE)
TAG_PREFIX = "libhegel/"
MONOREPO_SINCE = (0, 47, 2)


def git(*args: str) -> None:
    subprocess.run(["git", *args], check=True, cwd=ROOT)


def get_pinned_version() -> str:
    m = VERSION_RE.search(LOADER.read_text(encoding="utf-8"))
    assert m is not None, "could not find `let version` in loader.ml"
    return m.group(1)


def version_of_tag(tag: str) -> str:
    """The `let version` form ("0.47.2") of a `libhegel/0.47.2` tag."""
    if not tag.startswith(TAG_PREFIX):
        sys.exit(f"expected a {TAG_PREFIX}<version> tag, got {tag!r}")
    return tag[len(TAG_PREFIX):]


def release_url(version: str) -> str:
    """The GitHub release page for libhegel `version`.

    Mirrors `release_source` in scripts/update-checksums.py: releases before
    0.47.2 are on hegeldev/hegel-rust under `libhegel-v<version>` tags.
    """
    if tuple(int(p) for p in version.split(".")) < MONOREPO_SINCE:
        return f"https://github.com/hegeldev/hegel-rust/releases/tag/libhegel-v{version}"
    return f"https://github.com/{LIBHEGEL_REPO}/releases/tag/{TAG_PREFIX}{version}"


def resolve_latest() -> str:
    # The newest `libhegel/` tag with a GitHub release. Only libhegel tags in
    # the monorepo carry one, but filter on the prefix rather than trusting
    # "latest" to be a libhegel release.
    tags = subprocess.run(
        ["gh", "release", "list", "--repo", LIBHEGEL_REPO,
         "--exclude-drafts", "--exclude-pre-releases",
         "--json", "tagName", "--jq", ".[].tagName"],
        check=True, capture_output=True, text=True,
    ).stdout.split()
    for tag in tags:
        if tag.startswith(TAG_PREFIX):
            return tag
    sys.exit(f"no {TAG_PREFIX}<version> release found in {LIBHEGEL_REPO}")


def set_pinned_version(version: str) -> None:
    text = LOADER.read_text(encoding="utf-8")
    new_text, n = VERSION_RE.subn(f'let version = "{version}"', text, count=1)
    assert n == 1, "expected exactly one `let version` line in loader.ml"
    LOADER.write_text(new_text, encoding="utf-8")


def bump(requested_tag: str) -> None:
    current = get_pinned_version()
    target_tag = requested_tag or resolve_latest()
    target = version_of_tag(target_tag)

    if target == current:
        print(f"Already pinned to {target_tag}; nothing to do.")
        return

    # Pin the new version, then regenerate the checksum table.
    # update-checksums.py reads the version back out of loader.ml (and derives
    # the release tag from it), so it must be written first.
    set_pinned_version(target)
    subprocess.run([sys.executable, str(UPDATE_CHECKSUMS)], check=True, cwd=ROOT)

    current_url = release_url(current)
    new_url = release_url(target)

    RELEASE_MD.write_text(
        "RELEASE_TYPE: patch\n\n"
        "This patch bumps our pinned libhegel "
        f"([hegeldev/hegel](https://github.com/{LIBHEGEL_REPO})) from "
        f"[{current}]({current_url}) to [{target}]({new_url}).\n",
        encoding="utf-8",
    )

    app_id = os.environ["HEGEL_RELEASE_APP_ID"]
    git("config", "user.name", "hegel-release[bot]")
    git("config", "user.email", f"{app_id}+hegel-release[bot]@users.noreply.github.com")

    # A fixed branch that we force-push: an already-open PR is updated in place
    # to the newest version rather than stacking up a new PR per release.
    git("checkout", "-B", BRANCH)
    git("add", str(LOADER), str(RELEASE_MD))
    git("commit", "-m", f"Bump pinned libhegel to {target}")
    git("push", "--force", "origin", BRANCH)

    title = f"Bump pinned `libhegel` to `{target}`"
    body = (
        f"This PR bumps our pinned `libhegel` version to `{target}` "
        f"([`{target_tag}`]({new_url})).\n"
        "\n"
        "---\n"
        "\n"
        f"*This PR was automatically generated by [bump-hegel-rust.yml]({WORKFLOW_URL}).*"
    )

    # Only create a PR if one doesn't already exist for this branch; the
    # force-push above already updated an open PR's contents.
    result = subprocess.run(
        ["gh", "pr", "list", "--head", BRANCH, "--state", "open", "--json", "number"],
        capture_output=True,
        text=True,
        cwd=ROOT,
    )
    has_open_pr = result.returncode == 0 and result.stdout.strip() not in ("", "[]")

    if has_open_pr:
        subprocess.run(
            ["gh", "pr", "edit", BRANCH, "--title", title, "--body", body],
            check=True,
            cwd=ROOT,
        )
    else:
        subprocess.run(
            ["gh", "pr", "create", "--title", title, "--body", body],
            check=True,
            cwd=ROOT,
        )


if __name__ == "__main__":
    # An optional argument pins that exact `libhegel/<version>` tag; with none
    # we take the latest libhegel release. The repository_dispatch trigger
    # passes client_payload.tag.
    bump(sys.argv[1] if len(sys.argv) > 1 else "")
