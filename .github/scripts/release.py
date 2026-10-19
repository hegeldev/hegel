from __future__ import annotations

import argparse
import os
import re
import subprocess
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent


@dataclass(frozen=True)
class Component:
    """A separately versioned part of the monorepo.

    `path` is the component's directory relative to the repo root. It doubles
    as the tag prefix: a release of version X.Y.Z is tagged `<path>/X.Y.Z`.
    `manifests` are the Cargo manifests that carry the component's version
    (the first is the one its version is read from); `source_dirs` are the
    directories whose changes require a `<path>/RELEASE.md` on a PR.
    """

    path: str
    manifests: tuple[str, ...]
    source_dirs: tuple[str, ...]

    @property
    def release_file(self) -> str:
        return f"{self.path}/RELEASE.md"

    @property
    def changelog(self) -> str:
        return f"{self.path}/CHANGELOG.md"

    def tag(self, version: str) -> str:
        return f"{self.path}/{version}"

    def manifest(self) -> Path:
        return ROOT / self.manifests[0]


# libhegel (the `hegeltest-c` crate) and the Rust frontend (`hegeltest` and
# `hegeltest-macros`) carry independent version numbers, because a change can
# be breaking for one without being breaking for the other (a breaking C ABI
# change is a minor bump for libhegel but only a patch for the Rust frontend).
# Each is tagged separately as `<path>/<version>`; the libhegel tag also
# carries the GitHub release the libhegel binaries are uploaded to.
LIBHEGEL = Component(
    path="libhegel",
    manifests=("libhegel/Cargo.toml",),
    source_dirs=("libhegel/src/",),
)
RUST = Component(
    path="frontends/rust",
    manifests=("frontends/rust/Cargo.toml", "frontends/rust/macros/Cargo.toml"),
    source_dirs=("frontends/rust/src/", "frontends/rust/macros/"),
)
COMPONENTS = [LIBHEGEL, RUST]


def libhegel_release_title(version: str) -> str:
    return f"libhegel v{version}"


# Files the release commit reads, rewrites, and stages. These are validated by
# `check` on every PR so removing one fails fast instead of breaking the actual
# release — which only ever runs the `release` subcommand on a push to main.
RELEASE_PATHS = [
    "Cargo.lock",
    *(m for c in COMPONENTS for m in c.manifests),
    *(c.changelog for c in COMPONENTS),
]


def git(*args: str, cwd: Path | None = None) -> None:
    subprocess.run(["git", *args], check=True, cwd=cwd)


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
    parts = current.split(".")
    major, minor, patch = int(parts[0]), int(parts[1]), int(parts[2])

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


def current_version(cargo_toml: Path) -> str:
    """The `version = "..."` of the package manifest at `cargo_toml`."""
    m = re.search(r'^version = "([^"]+)"', cargo_toml.read_text(), re.MULTILINE)
    return m.group(1)


def set_version(cargo_toml: Path, new_version: str) -> None:
    text = cargo_toml.read_text()
    new_text = re.sub(
        r'^version = "[^"]+"',
        f'version = "{new_version}"',
        text,
        count=1,
        flags=re.MULTILINE,
    )
    cargo_toml.write_text(new_text)


def bump_internal_path_deps(cargo_toml: Path, versions: dict[str, str]) -> None:
    """Bump each internal (path) dependency pinned with `=` to its new version.

    Internal crates are pinned exactly (`version = "=X.Y.Z"`) and declared with
    a `path = ...`. `versions` maps a crate name to the version its pin should
    move to. Requiring both an exact pin and a `path =` (rather than trusting a
    hardcoded crate list) means a change in dependency direction can't silently
    leave a stale pin behind — which is exactly how the inversion refactor broke
    the release: the frontend crate gained a `hegeltest-c` path dependency that
    the old hardcoded list never touched. The pin is keyed by crate name so each
    follows its own crate's version.
    """
    lines = cargo_toml.read_text().splitlines(keepends=True)
    out = []
    for line in lines:
        match = re.match(r"\s*([A-Za-z0-9_-]+)\s*=", line)
        if (
            match
            and match.group(1) in versions
            and "path =" in line
            and re.search(r'version = "=[^"]+"', line)
        ):
            line = re.sub(
                r'version = "=[^"]+"',
                f'version = "={versions[match.group(1)]}"',
                line,
            )
        out.append(line)
    cargo_toml.write_text("".join(out))


def apply_version_bump(root: Path, rust_version: str, c_version: str) -> None:
    """Rewrite the workspace manifests to their new versions.

    The Rust frontend's manifests (`hegeltest` and `hegeltest-macros`) move to
    `rust_version`; libhegel's moves to `c_version`. Internal path-dependency
    pins follow the crate they point at. Pure file rewriting, factored out of
    `release()` so it can be tested without the publish/git/gh machinery.
    """
    versions = {"hegeltest-macros": rust_version, "hegeltest-c": c_version}

    for rel in RUST.manifests:
        set_version(root / rel, rust_version)
    for rel in LIBHEGEL.manifests:
        set_version(root / rel, c_version)

    for component in COMPONENTS:
        for rel in component.manifests:
            bump_internal_path_deps(root / rel, versions)


def add_changelog(path: Path, *, version: str, content: str) -> None:
    date = datetime.now(timezone.utc).strftime("%Y-%m-%d")
    entry = f"## {version} - {date}\n\n{content}"

    existing = path.read_text()
    assert existing.startswith("# Changelog")
    rest = existing.removeprefix("# Changelog")
    path.write_text(f"# Changelog\n\n{entry}{rest}")


def plan_release(
    rust_current: str,
    c_current: str,
    rust_release: tuple[str, str] | None,
    c_release: tuple[str, str] | None,
) -> tuple[str, str, str, str | None]:
    """Decide the new versions and changelog bodies for a release.

    `rust_release`/`c_release` are `(release_type, content)` pairs parsed from
    `frontends/rust/RELEASE.md` / `libhegel/RELEASE.md`, or None when that file
    is absent. At least one must be present (the release only runs when one
    exists).

    The two components version independently. libhegel bumps by its own
    release file. The Rust frontend bumps by its release file when present;
    otherwise — when only libhegel changed — it takes a patch bump because
    `hegeltest` pins `hegeltest-c` exactly and must be republished against the
    new engine, and its changelog gets an auto-generated dependency-bump entry.
    That entry is only correct when there are no functional frontend changes
    (see the changelog skill).

    Returns `(rust_version, c_version, rust_content, c_content)`; `c_content`
    is None (and `c_version` unchanged) when there is no libhegel/RELEASE.md.
    """
    assert rust_release is not None or c_release is not None

    if c_release is not None:
        c_version = bump_version(c_current, c_release[0])
        c_content = c_release[1]
    else:
        c_version = c_current
        c_content = None

    if rust_release is not None:
        rust_version = bump_version(rust_current, rust_release[0])
        rust_content = rust_release[1]
    else:
        rust_version = bump_version(rust_current, "patch")
        rust_content = (
            f"This release updates the `hegeltest-c` dependency to {c_version}."
        )

    return rust_version, c_version, rust_content, c_content


def check(base_ref: str) -> None:
    missing = [rel for rel in RELEASE_PATHS if not (ROOT / rel).exists()]
    if missing:
        raise ValueError(
            "release.py would fail: these paths it stages no longer exist: "
            + ", ".join(missing)
        )

    output = subprocess.check_output(
        ["git", "diff", "--name-only", f"origin/{base_ref}...HEAD"],
        text=True,
        cwd=ROOT,
    )
    changed_files = [line for line in output.splitlines() if line.strip()]

    def changed(component: Component) -> bool:
        return any(
            f.startswith(d) for f in changed_files for d in component.source_dirs
        )

    # A libhegel release auto-generates the Rust frontend's dependency-bump
    # entry, so when libhegel is releasing (its RELEASE.md is present) the
    # frontend's RELEASE.md is optional — the author writes one only when there
    # are functional frontend changes to document. A frontend change with no
    # libhegel release has no such fallback, so its RELEASE.md is required.
    c_release_present = (ROOT / LIBHEGEL.release_file).exists()

    require_release_file(
        base_ref,
        RUST.release_file,
        changed_files,
        [
            f"Every pull request changing the Rust frontend requires a "
            f"{RUST.release_file} file.",
            "You can find an example and instructions in "
            "frontends/rust/RELEASE-sample.md.",
        ],
        required=changed(RUST) and not c_release_present,
    )

    require_release_file(
        base_ref,
        LIBHEGEL.release_file,
        changed_files,
        [
            f"Every pull request changing libhegel requires a "
            f"{LIBHEGEL.release_file} file.",
            "You can find an example and instructions in "
            "frontends/rust/RELEASE-sample.md.",
        ],
        required=changed(LIBHEGEL),
    )


def require_release_file(
    base_ref: str,
    rel_path: str,
    changed_files: list[str],
    missing_lines: list[str],
    *,
    required: bool,
) -> None:
    """Validate a release file, erroring if it is `required` but absent.

    A present file is always validated (and guarded against being a leftover
    from a failed release); an absent file errors only when `required`.
    """
    release_file = ROOT / rel_path
    if not release_file.exists():
        if not required:
            return
        width = max(len(l) for l in missing_lines) + 6
        border = " ".join("*" * ((width + 1) // 2))
        empty = "*" + " " * (width - 2) + "*"
        inner = "\n".join("*" + l.center(width - 2) + "*" for l in missing_lines)
        pad = "\t"
        box = f"\n{pad}{border}\n{pad}{empty}\n{pad}{empty}\n"
        box += "\n".join(f"{pad}" + l for l in inner.split("\n"))
        box += f"\n{pad}{empty}\n{pad}{empty}\n{pad}{border}\n"
        raise ValueError(box)

    process = subprocess.run(
        ["git", "cat-file", "-e", f"origin/{base_ref}:{rel_path}"],
        capture_output=True,
        cwd=ROOT,
    )
    if process.returncode == 0 and rel_path not in changed_files:
        raise ValueError(
            f"{rel_path} already exists on {base_ref}. It's possible the CI job "
            "responsible for cutting a new release is in progress, or has failed."
        )

    parse_release_file(release_file)


def release() -> None:
    rust_path = ROOT / RUST.release_file
    c_path = ROOT / LIBHEGEL.release_file
    rust_release = parse_release_file(rust_path) if rust_path.exists() else None
    c_release = parse_release_file(c_path) if c_path.exists() else None
    assert rust_release is not None or c_release is not None

    rust_current = current_version(RUST.manifest())
    c_current = current_version(LIBHEGEL.manifest())
    rust_version, c_version, rust_content, c_content = plan_release(
        rust_current, c_current, rust_release, c_release
    )

    apply_version_bump(ROOT, rust_version, c_version)

    # regenerate the lockfile after version bump
    subprocess.run(["cargo", "update", "--workspace"], check=True, cwd=ROOT)

    add_changelog(ROOT / RUST.changelog, version=rust_version, content=rust_content)
    if c_content is not None:
        add_changelog(ROOT / LIBHEGEL.changelog, version=c_version, content=c_content)

    app_slug = os.environ["HEGEL_RELEASE_APP_SLUG"]
    bot_user_id = subprocess.check_output(
        ["gh", "api", f"/users/{app_slug}[bot]", "--jq", ".id"], text=True
    ).strip()
    git("config", "user.name", f"{app_slug}[bot]", cwd=ROOT)
    git(
        "config",
        "user.email",
        f"{bot_user_id}+{app_slug}[bot]@users.noreply.github.com",
        cwd=ROOT,
    )
    git("add", *RELEASE_PATHS, cwd=ROOT)
    if rust_release is not None:
        git("rm", RUST.release_file, cwd=ROOT)
    if c_release is not None:
        git("rm", LIBHEGEL.release_file, cwd=ROOT)

    bumped = [f"hegeltest to {rust_version}"]
    if c_release is not None:
        bumped.append(f"hegeltest-c to {c_version}")
    git(
        "commit",
        "-m",
        f"Bump {' and '.join(bumped)} and update changelog\n\n[skip ci]",
        cwd=ROOT,
    )

    # The Rust frontend bumps on every release, so its tag is always pushed.
    # The GitHub release carries the libhegel binaries, so it is the libhegel
    # release: it is tagged with the libhegel version and only cut when
    # libhegel changed. A frontend-only release publishes to crates.io,
    # updates the changelog and pushes its tag — no GitHub release.
    tags = release_tags(rust_version, c_version if c_release is not None else None)
    for tag in tags:
        git("tag", tag, cwd=ROOT)
    git("push", "origin", *tags, cwd=ROOT)
    if c_release is not None:
        subprocess.run(
            [
                "gh",
                "release",
                "create",
                LIBHEGEL.tag(c_version),
                "--draft",
                "--title",
                libhegel_release_title(c_version),
                "--notes",
                c_content,
            ],
            check=True,
            cwd=ROOT,
        )


def release_tags(rust_version: str, c_version: str | None) -> list[str]:
    """The tags a release commit gets: the Rust frontend's always, libhegel's
    when it released (`c_version` is None otherwise)."""
    tags = [RUST.tag(rust_version)]
    if c_version is not None:
        tags.append(LIBHEGEL.tag(c_version))
    return tags


def release_pr_details(tags: list[str]) -> tuple[str, str]:
    """Title and body for the fallback PR opened when the release push races a
    concurrent merge to main.

    `tags` is whatever release tags point at the release commit — the Rust
    frontend's `frontends/rust/<version>`, plus `libhegel/<version>` when
    libhegel released — all of which were pushed before the failed push to
    main.
    """
    joined = " and ".join(tags)
    title = f"Release {joined}"
    body = (
        f"The push to main after tagging {joined} failed because "
        f"main had diverged. The tags and crates.io publish succeeded.\n\n"
        f"This PR merges the release commit (version bump, changelog, "
        f"RELEASE.md removal) into main."
    )
    return title, body


def push_or_pr() -> None:
    version = current_version(RUST.manifest())

    result = subprocess.run(
        ["git", "push", "origin", "main"], cwd=ROOT
    )
    if result.returncode == 0:
        return

    print(f"Push to main failed, creating PR for release {RUST.tag(version)}")

    branch = f"release/rust-{version}"
    git("checkout", "-b", branch, cwd=ROOT)
    git("push", "origin", branch, cwd=ROOT)

    # Ensure the "skip release" label exists so check-release doesn't run on this PR
    subprocess.run(
        [
            "gh", "label", "create", "skip release",
            "--force",
            "--description", "Skip the release check on this PR",
        ],
        cwd=ROOT,
    )

    tags = subprocess.check_output(
        ["git", "tag", "--points-at", "HEAD"], cwd=ROOT, text=True
    ).split()
    title, body = release_pr_details(tags)
    subprocess.run(
        [
            "gh", "pr", "create",
            "--base", "main",
            "--head", branch,
            "--title", title,
            "--body", body,
            "--label", "skip release",
        ],
        check=True,
        cwd=ROOT,
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Release automation for the hegel monorepo.")
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
