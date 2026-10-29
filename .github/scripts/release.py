from __future__ import annotations

import argparse
import json
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
    `manifests` are the manifests that carry the component's version (Cargo
    manifests, npm's `package.json` and `package-lock.json`, a
    `CMakeLists.txt` with a `project(... VERSION x.y.z ...)` line, Maven's
    `pom.xml` files, MSBuild's `Directory.Build.props`, or dune's
    `dune-project` and the `.opam` files it generates; the first is the one
    its version is read from) and `crates` the
    packages those manifests declare, all at the component's version;
    `source_dirs` are the paths whose changes require a `<path>/RELEASE.md` on
    a PR.

    A frontend ships the engine (the Rust frontend pins the `hegeltest-c`
    crate exactly, the Python frontend compiles libhegel into its wheels, the
    TypeScript, Java, C# and OCaml frontends bundle the libhegel binaries, the
    C++ frontend builds the in-tree libhegel it is fetched with), so a libhegel release
    also patch-bumps every frontend without a RELEASE.md of its own, giving it
    `engine_bump_entry` (with `{version}` filled in) as its changelog entry.
    libhegel itself has no such entry. A frontend that records the engine
    version it ships in a generated source file names it in `engine_pin`;
    its `LIBHEGEL_VERSION = "..."` follows the libhegel version.

    `install_docs` are documents, such as a README, whose install snippets
    name the component's own version: a release rewrites every occurrence of
    the version it replaces to the new one.
    """

    path: str
    manifests: tuple[str, ...]
    crates: tuple[str, ...]
    source_dirs: tuple[str, ...]
    engine_bump_entry: str | None = None
    engine_pin: str | None = None
    install_docs: tuple[str, ...] = ()

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


# libhegel (the `hegeltest-c` crate) and the frontends carry independent
# version numbers, because a change can be breaking for one without being
# breaking for another (a breaking C ABI change is a minor bump for libhegel
# but only a patch for a frontend). Each is tagged separately as
# `<path>/<version>`; the libhegel tag also carries the GitHub release the
# libhegel binaries are uploaded to.
LIBHEGEL = Component(
    path="libhegel",
    manifests=("libhegel/Cargo.toml",),
    crates=("hegeltest-c",),
    source_dirs=("libhegel/src/",),
)
RUST = Component(
    path="frontends/rust",
    manifests=("frontends/rust/Cargo.toml", "frontends/rust/macros/Cargo.toml"),
    crates=("hegeltest", "hegeltest-macros"),
    source_dirs=("frontends/rust/src/", "frontends/rust/macros/"),
    engine_bump_entry="This release updates the `hegeltest-c` dependency to {version}.",
)
PYTHON = Component(
    path="frontends/python",
    manifests=("frontends/python/Cargo.toml",),
    crates=("hegel-python",),
    source_dirs=(
        "frontends/python/src/",
        "frontends/python/python/hegel/",
        "frontends/python/Cargo.toml",
        "frontends/python/pyproject.toml",
    ),
    engine_bump_entry="This release updates the bundled libhegel engine to {version}.",
)
TYPESCRIPT = Component(
    path="frontends/typescript",
    manifests=(
        "frontends/typescript/package.json",
        "frontends/typescript/package-lock.json",
    ),
    crates=("@hegeldev/hegel",),
    source_dirs=("frontends/typescript/src/", "frontends/typescript/package.json"),
    engine_bump_entry="This release updates the bundled libhegel engine to {version}.",
    engine_pin="frontends/typescript/src/libhegel-version.ts",
)
# The C++ frontend has no package index: a consumer fetches the monorepo at
# the `frontends/cpp/<version>` tag with CMake's FetchContent, so the tag is
# the release. Its version is the `project()` version in its CMakeLists.txt.
CPP = Component(
    path="frontends/cpp",
    manifests=("frontends/cpp/CMakeLists.txt",),
    crates=("hegel-cpp",),
    source_dirs=(
        "frontends/cpp/src/",
        "frontends/cpp/include/",
        "frontends/cpp/cmake/",
        "frontends/cpp/CMakeLists.txt",
    ),
    engine_bump_entry="This release updates the bundled libhegel engine to {version}.",
    install_docs=("frontends/cpp/README.md", "frontends/cpp/include/hegel/hegel.h"),
)
JAVA = Component(
    path="frontends/java",
    # The parent pom carries the version; each module pom repeats it in its
    # <parent> element, which must move with the release.
    manifests=(
        "frontends/java/pom.xml",
        "frontends/java/hegel-lowlevel/pom.xml",
        "frontends/java/hegel/pom.xml",
        "frontends/java/hegel-jna/pom.xml",
    ),
    crates=("hegel-parent", "hegel-lowlevel", "hegel", "hegel-jna"),
    source_dirs=(
        "frontends/java/shared/src/main/",
        "frontends/java/hegel-lowlevel/src/main/",
        "frontends/java/hegel/src/main/",
        "frontends/java/hegel-jna/src/main/",
        "frontends/java/pom.xml",
        "frontends/java/hegel-lowlevel/pom.xml",
        "frontends/java/hegel/pom.xml",
        "frontends/java/hegel-jna/pom.xml",
    ),
    engine_bump_entry="This release updates the bundled libhegel engine to {version}.",
    install_docs=("frontends/java/README.md",),
)
# The C# frontend's version is the <Version> of Directory.Build.props, which
# every project in the solution inherits: the `Hegel` package and the
# `Hegel.FSharp` package that depends on it at the same version.
CSHARP = Component(
    path="frontends/csharp",
    manifests=("frontends/csharp/Directory.Build.props",),
    crates=("Hegel", "Hegel.FSharp"),
    source_dirs=(
        "frontends/csharp/src/",
        "frontends/csharp/Directory.Build.props",
    ),
    engine_bump_entry="This release updates the bundled libhegel engine to {version}.",
    install_docs=("frontends/csharp/README.md",),
)
OCAML = Component(
    path="frontends/ocaml",
    manifests=(
        "frontends/ocaml/dune-project",
        "frontends/ocaml/hegel.opam",
        "frontends/ocaml/ppx_hegel_compat.opam",
        "frontends/ocaml/ppx_hegel_generator.opam",
        "frontends/ocaml/ppx_hegel_test.opam",
    ),
    crates=("hegel", "ppx_hegel_compat", "ppx_hegel_generator", "ppx_hegel_test"),
    source_dirs=(
        "frontends/ocaml/lib/",
        "frontends/ocaml/ppx/",
        "frontends/ocaml/template/",
        "frontends/ocaml/dune-project",
    ),
    engine_bump_entry="This release updates the bundled libhegel engine to {version}.",
)
FRONTENDS = [RUST, PYTHON, TYPESCRIPT, CPP, JAVA, CSHARP, OCAML]
COMPONENTS = [LIBHEGEL, *FRONTENDS]


def libhegel_release_title(version: str) -> str:
    return f"libhegel v{version}"


# Files the release commit reads, rewrites, and stages. These are validated by
# `check` on every PR so removing one fails fast instead of breaking the actual
# release — which only ever runs the `release` subcommand on a push to main.
RELEASE_PATHS = [
    "Cargo.lock",
    *(m for c in COMPONENTS for m in c.manifests),
    *(c.changelog for c in COMPONENTS),
    *(c.engine_pin for c in COMPONENTS if c.engine_pin is not None),
    *(doc for c in COMPONENTS for doc in c.install_docs),
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


CMAKE_PROJECT_VERSION = re.compile(
    r"^(project\(\s*[\w-]+\s+VERSION\s+)(\d+\.\d+\.\d+)", re.MULTILINE
)


def version_pattern(manifest: Path) -> tuple[str, str]:
    """The regex matching the version line of a text manifest, and the
    replacement line for a new version (with `{version}` to fill in)."""
    if manifest.name == "dune-project":
        return r"^\(version [^)]+\)", "(version {version})"
    if manifest.suffix == ".opam":
        return r'^version: "[^"]+"', 'version: "{version}"'
    return r'^version = "[^"]+"', 'version = "{version}"'


def current_version(manifest: Path) -> str:
    """The version of the package manifest at `manifest`: the `version = "..."`
    of a Cargo manifest, the `"version"` of an npm one, the `project()`
    version of a CMakeLists.txt, the first `<version>` of a Maven pom (the
    project's own in a parent pom, the parent reference in a module pom;
    `modelVersion` is a different tag), the `<Version>` of an MSBuild
    `.props` file, the `(version ...)` of a dune-project or the
    `version: "..."` of an opam file."""
    if manifest.suffix == ".json":
        return json.loads(manifest.read_text())["version"]
    if manifest.name == "CMakeLists.txt":
        m = CMAKE_PROJECT_VERSION.search(manifest.read_text())
        if m is None:
            raise ValueError(f"no project(... VERSION x.y.z ...) in {manifest}")
        return m.group(2)
    if manifest.suffix == ".xml":
        return re.search(r"<version>([^<]+)</version>", manifest.read_text()).group(1)
    if manifest.suffix == ".props":
        m = re.search(r"<Version>([^<]+)</Version>", manifest.read_text())
        if m is None:
            raise ValueError(f"no <Version> in {manifest}")
        return m.group(1)
    pattern, _ = version_pattern(manifest)
    m = re.search(pattern, manifest.read_text(), re.MULTILINE)
    return re.search(r"\d+\.\d+\.\d+", m.group(0)).group(0)


def set_version(manifest: Path, new_version: str) -> None:
    if manifest.name == "CMakeLists.txt":
        text = manifest.read_text()
        new_text, count = CMAKE_PROJECT_VERSION.subn(
            lambda m: m.group(1) + new_version, text, count=1
        )
        if count != 1:
            raise ValueError(f"no project(... VERSION x.y.z ...) in {manifest}")
        manifest.write_text(new_text)
        return
    if manifest.suffix == ".json":
        data = json.loads(manifest.read_text())
        data["version"] = new_version
        # package-lock.json repeats the root package's version under packages[""].
        root_package = data.get("packages", {}).get("")
        if root_package is not None:
            root_package["version"] = new_version
        manifest.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
        return
    if manifest.suffix == ".xml":
        manifest.write_text(
            re.sub(
                r"<version>[^<]+</version>",
                f"<version>{new_version}</version>",
                manifest.read_text(),
                count=1,
            )
        )
        return
    if manifest.suffix == ".props":
        text = manifest.read_text()
        new_text, count = re.subn(
            r"<Version>[^<]+</Version>", f"<Version>{new_version}</Version>", text, count=1
        )
        if count != 1:
            raise ValueError(f"no <Version> in {manifest}")
        manifest.write_text(new_text)
        return
    text = manifest.read_text()
    pattern, replacement = version_pattern(manifest)
    new_text = re.sub(
        pattern,
        replacement.format(version=new_version),
        text,
        count=1,
        flags=re.MULTILINE,
    )
    manifest.write_text(new_text)


def set_engine_pin(pin: Path, version: str) -> None:
    """Point a frontend's generated `LIBHEGEL_VERSION = "..."` at `version`."""
    text = pin.read_text()
    new_text, count = re.subn(
        r'LIBHEGEL_VERSION = "[^"]+"', f'LIBHEGEL_VERSION = "{version}"', text
    )
    if count != 1:
        raise ValueError(f"expected exactly one LIBHEGEL_VERSION in {pin}, found {count}")
    pin.write_text(new_text)


def set_install_doc_version(doc: Path, old_version: str, new_version: str) -> None:
    """Rewrite every mention of `old_version` in an install document to
    `new_version`. A document that no longer mentions it is an error, so a
    snippet that drifted from the released version fails the release rather
    than going stale."""
    text = doc.read_text()
    new_text, count = re.subn(
        rf"(?<![\d.]){re.escape(old_version)}(?![\d.])", new_version, text
    )
    if count == 0:
        raise ValueError(f"{doc} does not mention the version {old_version} it should pin")
    doc.write_text(new_text)


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


def apply_version_bump(root: Path, versions: dict[Component, str]) -> None:
    """Rewrite the workspace manifests to their new versions.

    `versions` maps each component being released to its new version; the
    others keep their current version. Internal path-dependency pins and the
    frontends' engine pins follow the crate they point at, whether or not
    that crate moved. Pure file rewriting, factored out of `release()` so it
    can be tested without the publish/git/gh machinery.
    """
    for component, version in versions.items():
        old_version = current_version(root / component.manifests[0])
        for rel in component.manifests:
            set_version(root / rel, version)
        for rel in component.install_docs:
            set_install_doc_version(root / rel, old_version, version)

    pins = {
        crate: versions.get(component) or current_version(root / component.manifests[0])
        for component in COMPONENTS
        for crate in component.crates
    }
    for component in COMPONENTS:
        for rel in component.manifests:
            if (root / rel).suffix == ".toml":
                bump_internal_path_deps(root / rel, pins)
        if component.engine_pin is not None:
            set_engine_pin(root / component.engine_pin, pins[LIBHEGEL.crates[0]])


def add_changelog(path: Path, *, version: str, content: str) -> None:
    date = datetime.now(timezone.utc).strftime("%Y-%m-%d")
    entry = f"## {version} - {date}\n\n{content}"

    existing = path.read_text()
    assert existing.startswith("# Changelog")
    rest = existing.removeprefix("# Changelog")
    path.write_text(f"# Changelog\n\n{entry}{rest}")


def plan_release(
    current: dict[Component, str],
    releases: dict[Component, tuple[str, str] | None],
) -> dict[Component, tuple[str, str]]:
    """Decide the new versions and changelog bodies for a release.

    `current` maps every component to its current version and `releases`
    maps it to the `(release_type, content)` pair parsed from its RELEASE.md,
    or None when that file is absent. At least one must be present (the
    release only runs when one exists).

    The components version independently: each bumps by its own release file.
    A frontend without one still takes a patch bump when libhegel releases,
    because it ships the engine (the Rust frontend pins `hegeltest-c` exactly
    and must be republished against it; the Python frontend compiles libhegel
    into its wheels), and its changelog gets the auto-generated
    `engine_bump_entry`. That entry is only correct when there are no
    functional frontend changes (see the changelog skill).

    Returns `{component: (new_version, changelog_content)}` for the
    components that release; the others are absent.
    """
    assert any(release is not None for release in releases.values())

    plan: dict[Component, tuple[str, str]] = {}
    libhegel = releases.get(LIBHEGEL)
    if libhegel is not None:
        plan[LIBHEGEL] = (bump_version(current[LIBHEGEL], libhegel[0]), libhegel[1])

    for frontend in FRONTENDS:
        release = releases.get(frontend)
        if release is not None:
            plan[frontend] = (bump_version(current[frontend], release[0]), release[1])
        elif libhegel is not None:
            assert frontend.engine_bump_entry is not None
            plan[frontend] = (
                bump_version(current[frontend], "patch"),
                frontend.engine_bump_entry.format(version=plan[LIBHEGEL][0]),
            )

    return plan


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

    # A libhegel release auto-generates each frontend's engine-bump entry, so
    # when libhegel is releasing (its RELEASE.md is present) a frontend's
    # RELEASE.md is optional — the author writes one only when there are
    # functional frontend changes to document. A frontend change with no
    # libhegel release has no such fallback, so its RELEASE.md is required.
    c_release_present = (ROOT / LIBHEGEL.release_file).exists()

    for component in COMPONENTS:
        required = changed(component)
        if component is not LIBHEGEL:
            required = required and not c_release_present
        require_release_file(
            base_ref,
            component.release_file,
            changed_files,
            [
                f"Every pull request changing {component.path} requires a "
                f"{component.release_file} file.",
                "You can find an example and instructions in CONTRIBUTING.md.",
            ],
            required=required,
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
    releases = {
        component: (
            parse_release_file(ROOT / component.release_file)
            if (ROOT / component.release_file).exists()
            else None
        )
        for component in COMPONENTS
    }
    assert any(release is not None for release in releases.values())

    current = {component: current_version(component.manifest()) for component in COMPONENTS}
    plan = plan_release(current, releases)
    versions = {component: version for component, (version, _) in plan.items()}

    apply_version_bump(ROOT, versions)

    # regenerate the lockfile after version bump
    subprocess.run(["cargo", "update", "--workspace"], check=True, cwd=ROOT)

    for component, (version, content) in plan.items():
        add_changelog(ROOT / component.changelog, version=version, content=content)

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
    for component, release in releases.items():
        if release is not None:
            git("rm", component.release_file, cwd=ROOT)

    tags = release_tags(versions)
    git("commit", "-m", release_commit_message(tags), cwd=ROOT)

    # Every released component gets its `<path>/<version>` tag. The GitHub
    # release carries the libhegel binaries, so it is the libhegel release:
    # it is tagged with the libhegel version and only cut when libhegel
    # changed. A frontend-only release publishes to its package index,
    # updates the changelog and pushes its tag — no GitHub release.
    for tag in tags:
        git("tag", tag, cwd=ROOT)
    git("push", "origin", *tags, cwd=ROOT)
    if LIBHEGEL in plan:
        c_version, c_content = plan[LIBHEGEL]
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


def release_tags(versions: dict[Component, str]) -> list[str]:
    """The tags a release commit gets: one per released component, libhegel
    first, then the frontends in `COMPONENTS` order."""
    return [c.tag(versions[c]) for c in COMPONENTS if c in versions]


def release_commit_message(tags: list[str]) -> str:
    return f"Release {' and '.join(tags)}\n\n[skip ci]"


def release_branch(tags: list[str]) -> str:
    """The branch for the fallback PR: the release tags, made branch-safe."""
    return "release/" + "+".join(tag.replace("/", "-") for tag in tags)


def release_pr_details(tags: list[str]) -> tuple[str, str]:
    """Title and body for the fallback PR opened when the release push races a
    concurrent merge to main.

    `tags` is whatever release tags point at the release commit — one
    `<path>/<version>` per released component — all of which were pushed
    before the failed push to main.
    """
    joined = " and ".join(tags)
    title = f"Release {joined}"
    body = (
        f"The push to main after tagging {joined} failed because "
        f"main had diverged. The tags and package publishes succeeded.\n\n"
        f"This PR merges the release commit (version bump, changelog, "
        f"RELEASE.md removal) into main."
    )
    return title, body


def push_or_pr() -> None:
    result = subprocess.run(
        ["git", "push", "origin", "main"], cwd=ROOT
    )
    if result.returncode == 0:
        return

    tags = subprocess.check_output(
        ["git", "tag", "--points-at", "HEAD"], cwd=ROOT, text=True
    ).split()
    print(f"Push to main failed, creating PR for release {' and '.join(tags)}")

    branch = release_branch(tags)
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
