#!/usr/bin/env python3
"""Tests for the version-rewriting half of release.py.

The publish/git/gh half of `release()` can only run in CI with real
credentials, but the file rewriting (`apply_version_bump`) is pure and is
exactly where the inversion refactor broke the release: the frontend crate
started depending on `hegeltest-c` via a `=`-pinned path dependency, and the
old hardcoded bump list never touched it, so `cargo update --workspace` blew
up with "failed to select a version for the requirement `hegeltest-c =
0.19.1`" after the package version had already moved to 0.19.2.
"""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import release
from release import LIBHEGEL, PYTHON, RUST

RUST_CARGO = """\
[package]
name = "hegeltest"
version = "0.19.1"
edition = "2024"

[dependencies]
hegeltest-macros = { version = "=0.19.1", path = "macros" }
hegeltest-c = { version = "=0.5.0", path = "../../libhegel", default-features = false, optional = true }
serde = { version = "1.0.103", features = ["derive"] }

[features]
static-engine = ["dep:hegeltest-c"]
"""

MACROS_CARGO = """\
[package]
name = "hegeltest-macros"
version = "0.19.1"
edition = "2024"

[dependencies]
syn = { version = "2.0", features = ["full"] }
"""

# libhegel does not depend on hegeltest. It must keep bumping cleanly (its
# package version moves; it has no internal deps to touch) so that a future
# re-inversion is the only thing that re-adds one.
C_CARGO = """\
[package]
name = "hegeltest-c"
version = "0.5.0"
edition = "2024"

[dependencies]
ciborium = "0.2.2"
serde = { version = "1.0.103", features = ["derive"] }
"""

# The Python extension builds libhegel from the tree, so its dependency is an
# unpinned path: nothing to bump but its own version.
PYTHON_CARGO = """\
[package]
name = "hegel-python"
version = "0.1.0"
edition = "2024"

[dependencies]
pyo3 = { version = "0.29", features = ["num-bigint"] }
hegeltest-c = { path = "../../libhegel", default-features = false, features = ["std"] }
"""

PATCH = ("patch", "This patch does a thing.")
MINOR = ("minor", "This release breaks the C ABI.")


class ApplyVersionBumpTest(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        for rel, text in [
            ("frontends/rust/Cargo.toml", RUST_CARGO),
            ("frontends/rust/macros/Cargo.toml", MACROS_CARGO),
            ("libhegel/Cargo.toml", C_CARGO),
            ("frontends/python/Cargo.toml", PYTHON_CARGO),
        ]:
            path = self.root / rel
            path.parent.mkdir(parents=True)
            path.write_text(text)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def read(self, rel: str) -> str:
        return (self.root / rel).read_text()

    def test_bumps_each_package_to_its_own_version(self) -> None:
        # The Rust frontend (hegeltest + macros), libhegel and the Python
        # frontend version independently.
        release.apply_version_bump(
            self.root, {RUST: "0.19.2", LIBHEGEL: "0.6.0", PYTHON: "0.1.1"}
        )
        for rel in RUST.manifests:
            self.assertIn('version = "0.19.2"', self.read(rel), rel)
            self.assertNotIn('version = "0.19.1"', self.read(rel), rel)
        self.assertIn('version = "0.6.0"', self.read("libhegel/Cargo.toml"))
        self.assertNotIn('version = "0.5.0"', self.read("libhegel/Cargo.toml"))
        self.assertIn('version = "0.1.1"', self.read("frontends/python/Cargo.toml"))

    def test_leaves_unreleased_components_alone(self) -> None:
        release.apply_version_bump(self.root, {PYTHON: "0.1.1"})
        self.assertEqual(self.read("frontends/rust/Cargo.toml"), RUST_CARGO)
        self.assertEqual(self.read("frontends/rust/macros/Cargo.toml"), MACROS_CARGO)
        self.assertEqual(self.read("libhegel/Cargo.toml"), C_CARGO)
        self.assertIn('version = "0.1.1"', self.read("frontends/python/Cargo.toml"))

    def test_bumps_internal_path_dependency_pins_per_crate(self) -> None:
        # Each internal path dep in the hegeltest manifest must follow its own
        # crate's version, or `cargo update --workspace` fails: the macros pin
        # tracks the frontend version, the hegeltest-c pin the libhegel version.
        release.apply_version_bump(self.root, {RUST: "0.19.2", LIBHEGEL: "0.6.0"})
        rust_text = self.read("frontends/rust/Cargo.toml")
        self.assertIn(
            'hegeltest-macros = { version = "=0.19.2", path = "macros" }',
            rust_text,
        )
        self.assertIn(
            'hegeltest-c = { version = "=0.6.0", path = "../../libhegel", '
            "default-features = false, optional = true }",
            rust_text,
        )

    def test_pins_follow_an_unreleased_crate_too(self) -> None:
        # A frontend-only release leaves libhegel where it is, and the pin on
        # it must stay at that version rather than be dropped or moved.
        release.apply_version_bump(self.root, {RUST: "0.19.2"})
        rust_text = self.read("frontends/rust/Cargo.toml")
        self.assertIn('hegeltest-macros = { version = "=0.19.2", path = "macros" }', rust_text)
        self.assertIn('hegeltest-c = { version = "=0.5.0", path = "../../libhegel"', rust_text)

    def test_leaves_external_and_unpinned_dependencies_untouched(self) -> None:
        release.apply_version_bump(
            self.root, {RUST: "0.19.2", LIBHEGEL: "0.6.0", PYTHON: "0.1.1"}
        )
        rust_text = self.read("frontends/rust/Cargo.toml")
        # External deps carry no `path =`, so they must be left alone — and the
        # `static-engine` feature reference is not a version pin either.
        self.assertIn('serde = { version = "1.0.103", features = ["derive"] }', rust_text)
        self.assertIn('static-engine = ["dep:hegeltest-c"]', rust_text)
        # The Python crate's path dependency carries no pin to move.
        self.assertIn(
            'hegeltest-c = { path = "../../libhegel", default-features = false, features = ["std"] }',
            self.read("frontends/python/Cargo.toml"),
        )


CURRENT = {RUST: "0.19.1", LIBHEGEL: "0.5.0", PYTHON: "0.1.0"}


class PlanReleaseTest(unittest.TestCase):
    def test_rust_only_bumps_rust_and_leaves_the_rest(self) -> None:
        plan = release.plan_release(CURRENT, {RUST: PATCH})
        self.assertEqual(plan, {RUST: ("0.19.2", "This patch does a thing.")})

    def test_python_only_bumps_python_and_leaves_the_rest(self) -> None:
        plan = release.plan_release(CURRENT, {PYTHON: PATCH})
        self.assertEqual(plan, {PYTHON: ("0.1.1", "This patch does a thing.")})

    def test_c_only_patch_bumps_every_frontend_with_an_engine_entry(self) -> None:
        plan = release.plan_release(CURRENT, {LIBHEGEL: ("patch", "This patch tweaks the C ABI.")})
        self.assertEqual(
            plan,
            {
                LIBHEGEL: ("0.5.1", "This patch tweaks the C ABI."),
                RUST: ("0.19.2", "This release updates the `hegeltest-c` dependency to 0.5.1."),
                PYTHON: ("0.1.1", "This release updates the bundled libhegel engine to 0.5.1."),
            },
        )

    def test_c_minor_is_only_a_frontend_patch(self) -> None:
        # The whole point of independent versions: a breaking C ABI change is a
        # minor bump for libhegel but only a patch for each frontend.
        plan = release.plan_release(CURRENT, {LIBHEGEL: MINOR})
        self.assertEqual(plan[LIBHEGEL], ("0.6.0", "This release breaks the C ABI."))
        self.assertEqual(plan[RUST][0], "0.19.2")
        self.assertEqual(plan[PYTHON][0], "0.1.1")

    def test_a_frontend_release_file_overrides_the_engine_entry(self) -> None:
        plan = release.plan_release(CURRENT, {RUST: PATCH, LIBHEGEL: MINOR})
        self.assertEqual(plan[RUST], ("0.19.2", "This patch does a thing."))
        self.assertEqual(plan[LIBHEGEL], ("0.6.0", "This release breaks the C ABI."))
        self.assertEqual(
            plan[PYTHON],
            ("0.1.1", "This release updates the bundled libhegel engine to 0.6.0."),
        )

    def test_absent_entries_count_as_no_release_file(self) -> None:
        plan = release.plan_release(CURRENT, {RUST: None, LIBHEGEL: None, PYTHON: PATCH})
        self.assertEqual(list(plan), [PYTHON])


class CurrentVersionTest(unittest.TestCase):
    def test_reads_the_package_version(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            manifest = Path(tmp) / "Cargo.toml"
            manifest.write_text(RUST_CARGO)
            self.assertEqual(release.current_version(manifest), "0.19.1")


class ComponentTest(unittest.TestCase):
    def test_tags_are_path_slash_version(self) -> None:
        self.assertEqual(RUST.tag("0.23.3"), "frontends/rust/0.23.3")
        self.assertEqual(LIBHEGEL.tag("0.24.0"), "libhegel/0.24.0")
        self.assertEqual(PYTHON.tag("0.2.0"), "frontends/python/0.2.0")

    def test_release_files_live_in_the_component(self) -> None:
        for component in release.COMPONENTS:
            self.assertEqual(component.release_file, f"{component.path}/RELEASE.md")
            self.assertEqual(component.changelog, f"{component.path}/CHANGELOG.md")

    def test_every_frontend_has_an_engine_bump_entry(self) -> None:
        self.assertIsNone(LIBHEGEL.engine_bump_entry)
        for frontend in release.FRONTENDS:
            self.assertIn("{version}", frontend.engine_bump_entry or "")

    def test_release_paths_cover_every_manifest_and_changelog(self) -> None:
        for component in release.COMPONENTS:
            for manifest in component.manifests:
                self.assertIn(manifest, release.RELEASE_PATHS)
            self.assertIn(component.changelog, release.RELEASE_PATHS)
        self.assertIn("Cargo.lock", release.RELEASE_PATHS)

    def test_release_paths_exist_in_the_checkout(self) -> None:
        for rel in release.RELEASE_PATHS:
            self.assertTrue((release.ROOT / rel).exists(), rel)

    def test_crates_match_the_manifests(self) -> None:
        for component in release.COMPONENTS:
            names = {
                release_name(release.ROOT / manifest) for manifest in component.manifests
            }
            self.assertEqual(names, set(component.crates), component.path)


def release_name(manifest: Path) -> str:
    for line in manifest.read_text().splitlines():
        if line.startswith("name = "):
            return line.split('"')[1]
    raise AssertionError(f"no package name in {manifest}")


class ReleaseTagsTest(unittest.TestCase):
    def test_rust_only_release_tags_only_the_rust_frontend(self) -> None:
        self.assertEqual(release.release_tags({RUST: "0.23.3"}), ["frontends/rust/0.23.3"])

    def test_c_release_tags_libhegel_first_then_the_frontends(self) -> None:
        self.assertEqual(
            release.release_tags({PYTHON: "0.2.0", RUST: "0.23.3", LIBHEGEL: "0.24.0"}),
            ["libhegel/0.24.0", "frontends/rust/0.23.3", "frontends/python/0.2.0"],
        )

    def test_libhegel_release_is_titled_after_libhegel(self) -> None:
        self.assertEqual(release.libhegel_release_title("0.24.0"), "libhegel v0.24.0")

    def test_commit_message_names_the_tags_and_skips_ci(self) -> None:
        message = release.release_commit_message(["libhegel/0.24.0", "frontends/rust/0.23.3"])
        self.assertEqual(
            message, "Release libhegel/0.24.0 and frontends/rust/0.23.3\n\n[skip ci]"
        )

    def test_release_branch_is_branch_safe(self) -> None:
        branch = release.release_branch(["libhegel/0.24.0", "frontends/python/0.2.0"])
        self.assertEqual(branch, "release/libhegel-0.24.0+frontends-python-0.2.0")


class ReleasePrDetailsTest(unittest.TestCase):
    def test_rust_only_release_names_its_tag(self) -> None:
        title, body = release.release_pr_details(["frontends/rust/0.23.3"])
        self.assertEqual(title, "Release frontends/rust/0.23.3")
        self.assertIn("after tagging frontends/rust/0.23.3 failed", body)
        self.assertIn("The tags and package publishes succeeded.", body)

    def test_c_release_names_every_tag(self) -> None:
        title, body = release.release_pr_details(
            ["libhegel/0.24.0", "frontends/rust/0.23.3", "frontends/python/0.2.0"]
        )
        self.assertEqual(
            title, "Release libhegel/0.24.0 and frontends/rust/0.23.3 and frontends/python/0.2.0"
        )
        self.assertIn("after tagging libhegel/0.24.0 and frontends/rust/0.23.3", body)


if __name__ == "__main__":
    unittest.main()
