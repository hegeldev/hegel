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
    `pom.xml` files, or dune's `dune-project` and the `.opam` files it
    generates; the first is the one its version is read from) and `crates` the
    packages those manifests declare, all at the component's version;
    `source_dirs` are the paths whose changes require a release file
    (`<path>/RELEASE/<slug>.md`, one per pull request so that two pull
    requests releasing the same component never conflict in the merge
    queue) on a PR.

    A frontend ships the engine (the Rust frontend pins the `hegeltest-c`
    crate exactly, the Python frontend compiles libhegel into its wheels, the
    TypeScript, Java and OCaml frontends bundle the libhegel binaries, the C++
    frontend builds the in-tree libhegel it is fetched with), so a libhegel release
    also patch-bumps every frontend without a release file of its own, giving it
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
    def release_dir(self) -> str:
        return f"{self.path}/RELEASE"

    @property
    def legacy_release_file(self) -> str:
        """The single per-component release file the directory replaced. It
        is still released when found on main, so a pull request written
        before the change releases as intended, but `check` refuses new ones:
        two pull requests adding it conflict with each other."""
        return f"{self.path}/RELEASE.md"

    def is_release_file(self, rel: str) -> bool:
        return rel == self.legacy_release_file or (
            rel.startswith(self.release_dir + "/") and rel.endswith(".md")
        )

    def release_files(self, root: Path) -> list[str]:
        """The component's pending release files in the tree at `root`,
        relative to it: the legacy file if it is still there, then the
        `RELEASE/<slug>.md` files in name order."""
        files = []
        if (root / self.legacy_release_file).is_file():
            files.append(self.legacy_release_file)
        release_dir = root / self.release_dir
        if release_dir.is_dir():
            files.extend(
                f"{self.release_dir}/{path.name}"
                for path in sorted(release_dir.iterdir())
                if path.is_file() and path.suffix == ".md"
            )
        return files

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
FRONTENDS = [RUST, PYTHON, TYPESCRIPT, CPP, JAVA, OCAML]
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
    `modelVersion` is a different tag), the `(version ...)` of a dune-project
    or the `version: "..."` of an opam file."""
    if manifest.suffix == ".json":
        return json.loads(manifest.read_text())["version"]
    if manifest.name == "CMakeLists.txt":
        m = CMAKE_PROJECT_VERSION.search(manifest.read_text())
        if m is None:
            raise ValueError(f"no project(... VERSION x.y.z ...) in {manifest}")
        return m.group(2)
    if manifest.suffix == ".xml":
        return re.search(r"<version>([^<]+)</version>", manifest.read_text()).group(1)
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
    maps it to the `(release_type, content)` pair of its release files
    (combined by `combine_entries` when there are several), or None when it
    has none. At least one must be present (the release only runs when one
    exists).

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


RELEASE_TYPES = ("patch", "minor", "major")


def combine_entries(entries: list[tuple[str, str]]) -> tuple[str, str]:
    """Fold a component's release files into one release: the largest
    `RELEASE_TYPE` wins and the entries follow each other as paragraphs, in
    the order given (`Component.release_files` order, by file name)."""
    assert entries
    release_type = max((t for t, _ in entries), key=RELEASE_TYPES.index)
    return release_type, "\n\n".join(content for _, content in entries)


def pending_release_files(root: Path) -> dict[Component, list[str]]:
    """Every component with a release file in the tree, with its files."""
    pending = {c: c.release_files(root) for c in COMPONENTS}
    return {c: files for c, files in pending.items() if files}


def set_output(name: str, value: str) -> None:
    """Expose a value to later workflow steps, when running in one."""
    output = os.environ.get("GITHUB_OUTPUT")
    if output:
        with open(output, "a") as f:
            f.write(f"{name}={value}\n")


def pending() -> None:
    files = [f for fs in pending_release_files(ROOT).values() for f in fs]
    print("\n".join(files) if files else "No release files pending.")
    set_output("skip", "false" if files else "true")


def missing_release_file_box(lines: list[str]) -> str:
    width = max(len(l) for l in lines) + 6
    border = " ".join("*" * ((width + 1) // 2))
    empty = "*" + " " * (width - 2) + "*"
    inner = "\n".join("*" + l.center(width - 2) + "*" for l in lines)
    pad = "\t"
    box = f"\n{pad}{border}\n{pad}{empty}\n{pad}{empty}\n"
    box += "\n".join(f"{pad}" + l for l in inner.split("\n"))
    box += f"\n{pad}{empty}\n{pad}{empty}\n{pad}{border}\n"
    return box


def check_release_files(root: Path, changed_files: list[str]) -> list[str]:
    """The problems with a pull request's release files, given the files it
    changes relative to its base. Empty when the pull request is fine.

    Only the pull request's own release files count: release files that are
    already on main belong to pull requests merged ahead of it whose release
    has not landed yet (the release job runs after the merge's CI, and only
    the run for the latest push to main gets to release), and are neither an
    error nor this pull request's documentation of its changes.

    A libhegel release auto-generates each frontend's engine-bump entry, so
    when the pull request releases libhegel a frontend's release file is
    optional: the author writes one only when there are functional frontend
    changes to document. A frontend change with no libhegel release has no
    such fallback, so its release file is required.
    """
    present = [f for f in changed_files if (root / f).is_file()]
    own = {c: [f for f in present if c.is_release_file(f)] for c in COMPONENTS}

    def changed(component: Component) -> bool:
        return any(f.startswith(d) for f in changed_files for d in component.source_dirs)

    errors = []
    for component in COMPONENTS:
        if component.legacy_release_file in own[component]:
            errors.append(
                f"{component.legacy_release_file} is one file per component, so "
                "two pull requests releasing the component conflict in the merge "
                f"queue. Write {component.release_dir}/<slug>.md instead, named "
                "after the change."
            )
        for rel in own[component]:
            try:
                parse_release_file(root / rel)
            except ValueError as e:
                errors.append(f"{rel}: {e}")
        required = changed(component)
        if component is not LIBHEGEL:
            required = required and not own[LIBHEGEL]
        if required and not own[component]:
            errors.append(
                missing_release_file_box(
                    [
                        f"Every pull request changing {component.path} requires a "
                        f"{component.release_dir}/<slug>.md file.",
                        "You can find an example and instructions in CONTRIBUTING.md.",
                    ]
                )
            )
    return errors


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
    errors = check_release_files(ROOT, changed_files)
    if errors:
        raise ValueError("\n".join(errors))


def existing_remote_tags(tags: list[str]) -> list[str]:
    output = subprocess.check_output(
        ["git", "ls-remote", "--tags", "origin", *tags], text=True, cwd=ROOT
    )
    remote = {line.split("\t", 1)[1].removeprefix("refs/tags/") for line in output.splitlines() if "\t" in line}
    return [tag for tag in tags if tag in remote]


def release() -> None:
    """Release everything pending on the commit checked out, if it is still
    the head of main.

    The run for every push to main gets here, and two merges in quick
    succession give two runs whose trees both carry the first merge's release
    files. Pushing the release commit to main is what decides between them:
    it is a fast-forward only for the run whose commit is main's head, and
    tags are pushed and packages published only after it succeeds. The other
    run's push is rejected and it stops having changed nothing; the winner's
    tree is a superset of its release files, so nothing is lost. A failed
    release leaves its files on main for the next push's run to pick up.
    """
    pending = pending_release_files(ROOT)
    if not pending:
        print("No release files pending.")
        set_output("released", "false")
        return

    releases: dict[Component, tuple[str, str] | None] = {
        component: (
            combine_entries([parse_release_file(ROOT / rel) for rel in pending[component]])
            if component in pending
            else None
        )
        for component in COMPONENTS
    }

    current = {component: current_version(component.manifest()) for component in COMPONENTS}
    plan = plan_release(current, releases)
    versions = {component: version for component, (version, _) in plan.items()}
    tags = release_tags(versions)

    existing = existing_remote_tags(tags)
    if existing:
        raise ValueError(
            f"already tagged on origin: {' and '.join(existing)}. The version bump "
            "of an earlier release is missing from this tree."
        )

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
    for files in pending.values():
        git("rm", "--quiet", *files, cwd=ROOT)

    git("commit", "-m", release_commit_message(tags), cwd=ROOT)

    pushed = subprocess.run(["git", "push", "origin", "HEAD:refs/heads/main"], cwd=ROOT)
    if pushed.returncode != 0:
        print(
            "main has moved on since this run's commit, so the run for its new "
            f"head releases instead. Nothing was tagged or published for {' and '.join(tags)}."
        )
        set_output("released", "false")
        return

    # Every released component gets its `<path>/<version>` tag. The GitHub
    # release carries the libhegel binaries, so it is the libhegel release:
    # it is tagged with the libhegel version and only cut when libhegel
    # changed. A frontend-only release publishes to its package index,
    # updates the changelog and pushes its tag — no GitHub release.
    # None of the tags is on origin (checked above), so a local one of the
    # same name is stale and is overwritten.
    for tag in tags:
        git("tag", "--force", tag, cwd=ROOT)
    git("push", "--atomic", "origin", *tags, cwd=ROOT)
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
    set_output("released", "true")


def release_tags(versions: dict[Component, str]) -> list[str]:
    """The tags a release commit gets: one per released component, libhegel
    first, then the frontends in `COMPONENTS` order."""
    return [c.tag(versions[c]) for c in COMPONENTS if c in versions]


def release_commit_message(tags: list[str]) -> str:
    return f"Release {' and '.join(tags)}\n\n[skip ci]"


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Release automation for the hegel monorepo.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    check_parser = subparsers.add_parser("check")
    check_parser.add_argument("base_ref", help="Git ref to diff against.")
    subparsers.add_parser("pending")
    subparsers.add_parser("release")

    args = parser.parse_args()
    if args.command == "check":
        check(args.base_ref)
    elif args.command == "pending":
        pending()
    elif args.command == "release":
        release()
