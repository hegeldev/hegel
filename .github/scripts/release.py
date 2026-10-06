"""Release automation for hegel-python.

`check <base_ref>` runs on every pull request: a PR that changes what ships
(the extension, the Python package, or the build manifests) must carry a
valid RELEASE.md. `release` runs on pushes to main that carry a RELEASE.md: it
bumps the version, moves the RELEASE.md text into CHANGELOG.md, commits, tags,
and cuts a GitHub release. `push-or-pr` then pushes the release commit to main,
falling back to a PR if main moved underneath it.

The version lives only in Cargo.toml: pyproject.toml declares it dynamic, so
maturin reads it from there, and `hegel.__version__` comes from the installed
metadata.
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent

# Changes under these paths alter what a user installs, so they need a
# RELEASE.md. Cargo.lock is included because it pins the engine and every
# other dependency compiled into the extension.
SOURCE_PATHS = ["src/", "python/hegel/", "Cargo.toml", "Cargo.lock"]

# Files the release commit reads, rewrites, and stages. `check` makes sure
# they still exist and still parse, so a change that would break the release
# fails on its PR rather than after merging.
RELEASE_PATHS = ["Cargo.toml", "Cargo.lock", "CHANGELOG.md"]

CHANGELOG_HEADER = "# Changelog"


def git(*args: str) -> None:
    subprocess.run(["git", *args], check=True, cwd=ROOT)


def parse_release_file(path: Path) -> tuple[str, str]:
    text = path.read_text()
    first_line, _, rest = text.partition("\n")

    match = re.match(r"^RELEASE_TYPE: (major|minor|patch)$", first_line)
    if not match:
        raise ValueError(
            f"Expected RELEASE_TYPE: major|minor|patch, got {first_line!r}"
        )

    content = rest.strip()
    if not content:
        raise ValueError("Changelog cannot be empty.")

    return match.group(1), content


def bump_version(current: str, release_type: str) -> str:
    major, minor, patch = (int(p) for p in current.split("."))

    if release_type == "major":
        major += 1
        minor = 0
        patch = 0
    elif release_type == "minor":
        minor += 1
        patch = 0
    else:
        assert release_type == "patch"
        patch += 1

    return f"{major}.{minor}.{patch}"


# The `version = "..."` line of the [package] table. Cargo.toml has no other
# top-level `version =` line (dependency versions are inline tables), and
# [package] comes first, so the first match is the package version.
VERSION_LINE = re.compile(r'^version = "(\d+\.\d+\.\d+)"$', re.MULTILINE)


def current_version(cargo_toml: Path) -> str:
    m = VERSION_LINE.search(cargo_toml.read_text())
    if m is None:
        raise ValueError(f"Could not find the package version in {cargo_toml}")
    return m.group(1)


def set_version(cargo_toml: Path, new_version: str) -> None:
    text = cargo_toml.read_text()
    new_text, n = VERSION_LINE.subn(f'version = "{new_version}"', text, count=1)
    assert n == 1
    cargo_toml.write_text(new_text)


def unreleased_version(changelog: Path) -> str | None:
    """The version of a leading `## X.Y.Z - unreleased` section, if any.

    The package's first version was written up before any release existed.
    While that section is still marked unreleased, the next release ships that
    version rather than bumping past it.
    """
    text = changelog.read_text()
    if not text.startswith(CHANGELOG_HEADER):
        raise ValueError(f"{changelog} must start with {CHANGELOG_HEADER!r}")
    m = re.search(r"^## (.+)$", text, re.MULTILINE)
    if m is None:
        return None
    um = re.fullmatch(r"(\d+\.\d+\.\d+) - unreleased", m.group(1))
    return um.group(1) if um else None


def plan_version(current: str, release_type: str, unreleased: str | None) -> str:
    if unreleased is None:
        return bump_version(current, release_type)
    if unreleased != current:
        raise ValueError(
            f"CHANGELOG.md has an unreleased {unreleased} section but Cargo.toml "
            f"is at {current}."
        )
    return current


def add_changelog(path: Path, *, version: str, content: str) -> None:
    date = datetime.now(timezone.utc).strftime("%Y-%m-%d")
    existing = path.read_text()
    assert existing.startswith(CHANGELOG_HEADER)
    rest = existing.removeprefix(CHANGELOG_HEADER).lstrip("\n")

    unreleased = f"## {version} - unreleased\n"
    if rest.startswith(unreleased):
        # Date the unreleased section, adding this release's notes after the
        # text already written for it.
        section, sep, older = rest.removeprefix(unreleased).partition("\n## ")
        rest = f"## {version} - {date}\n\n{section.strip()}\n\n{content}\n{sep}{older}"
    else:
        rest = f"## {version} - {date}\n\n{content}\n\n{rest}"
    path.write_text(f"{CHANGELOG_HEADER}\n\n{rest.rstrip()}\n")


def missing_release_box(lines: list[str]) -> str:
    width = max(len(line) for line in lines) + 6
    border = " ".join("*" * ((width + 1) // 2))
    empty = "*" + " " * (width - 2) + "*"
    pad = "\t"
    box = [border, empty, empty]
    box += ["*" + line.center(width - 2) + "*" for line in lines]
    box += [empty, empty, border]
    return "\n" + "\n".join(pad + line for line in box) + "\n"


def check(base_ref: str) -> None:
    missing = [rel for rel in RELEASE_PATHS if not (ROOT / rel).exists()]
    if missing:
        raise ValueError(
            "release.py would fail: these paths it stages no longer exist: "
            + ", ".join(missing)
        )
    # Exercise the parsing the release relies on.
    plan_version(
        current_version(ROOT / "Cargo.toml"),
        "patch",
        unreleased_version(ROOT / "CHANGELOG.md"),
    )

    output = subprocess.check_output(
        ["git", "diff", "--name-only", f"origin/{base_ref}...HEAD"],
        text=True,
        cwd=ROOT,
    )
    changed_files = [line for line in output.splitlines() if line.strip()]

    release_file = ROOT / "RELEASE.md"
    source_changed = any(
        f.startswith(p) for f in changed_files for p in SOURCE_PATHS
    )

    if not release_file.exists():
        if not source_changed:
            return
        raise ValueError(
            missing_release_box(
                [
                    "Every pull request to hegel-python requires a RELEASE.md file.",
                    "You can find an example and instructions in RELEASE-sample.md.",
                ]
            )
        )

    process = subprocess.run(
        ["git", "cat-file", "-e", f"origin/{base_ref}:RELEASE.md"],
        capture_output=True,
        cwd=ROOT,
    )
    if process.returncode == 0 and "RELEASE.md" not in changed_files:
        raise ValueError(
            f"RELEASE.md already exists on {base_ref}. It's possible the CI job "
            "responsible for cutting a new release is in progress, or has failed."
        )

    parse_release_file(release_file)


def prepare_release(root: Path) -> tuple[str, str]:
    """Rewrite the version and changelog for the pending RELEASE.md.

    Pure file rewriting (plus the lockfile refresh), separate from the
    git/gh half of `release()` so it can be exercised locally. Returns the
    new version and the release notes.
    """
    release_type, content = parse_release_file(root / "RELEASE.md")
    cargo_toml = root / "Cargo.toml"
    changelog = root / "CHANGELOG.md"

    version = plan_version(
        current_version(cargo_toml), release_type, unreleased_version(changelog)
    )
    set_version(cargo_toml, version)
    # Record the new package version in Cargo.lock without touching any
    # dependency.
    subprocess.run(
        ["cargo", "update", "--workspace"], check=True, cwd=root
    )
    add_changelog(changelog, version=version, content=content)
    return version, content


def release() -> None:
    version, content = prepare_release(ROOT)

    app_slug = os.environ["HEGEL_RELEASE_APP_SLUG"]
    bot_user_id = subprocess.check_output(
        ["gh", "api", f"/users/{app_slug}[bot]", "--jq", ".id"], text=True
    ).strip()
    git("config", "user.name", f"{app_slug}[bot]")
    git(
        "config",
        "user.email",
        f"{bot_user_id}+{app_slug}[bot]@users.noreply.github.com",
    )
    git("add", *RELEASE_PATHS)
    git("rm", "RELEASE.md")
    git(
        "commit",
        "-m",
        f"Bump to version {version} and update changelog\n\n[skip ci]",
    )
    git("tag", f"v{version}")
    git("push", "origin", f"v{version}")

    subprocess.run(
        [
            "gh",
            "release",
            "create",
            f"v{version}",
            "--title",
            f"v{version}",
            "--notes",
            content,
        ],
        check=True,
        cwd=ROOT,
    )


def push_or_pr() -> None:
    version = current_version(ROOT / "Cargo.toml")

    result = subprocess.run(["git", "push", "origin", "main"], cwd=ROOT)
    if result.returncode == 0:
        return

    print(f"Push to main failed, creating PR for release v{version}")

    branch = f"release/v{version}"
    git("checkout", "-b", branch)
    git("push", "origin", branch)

    # Ensure the "skip release" label exists so check-release doesn't run on
    # this PR.
    subprocess.run(
        [
            "gh", "label", "create", "skip release",
            "--force",
            "--description", "Skip the release check on this PR",
        ],
        cwd=ROOT,
    )

    subprocess.run(
        [
            "gh", "pr", "create",
            "--base", "main",
            "--head", branch,
            "--title", f"Release v{version}",
            "--body",
            f"The push to main after tagging v{version} failed because main had "
            f"diverged. The tag and GitHub release succeeded.\n\n"
            f"This PR merges the release commit (version bump, changelog, "
            f"RELEASE.md removal) into main.",
            "--label", "skip release",
        ],
        check=True,
        cwd=ROOT,
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Release automation for hegel-python."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    check_parser = subparsers.add_parser("check")
    check_parser.add_argument("base_ref", help="Git ref to diff against.")
    subparsers.add_parser("release")
    subparsers.add_parser("push-or-pr")

    args = parser.parse_args()
    if args.command == "check":
        check(args.base_ref)
    elif args.command == "release":
        release()
    elif args.command == "push-or-pr":
        push_or_pr()
