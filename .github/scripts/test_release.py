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


class ApplyVersionBumpTest(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        for rel, text in [
            ("frontends/rust/Cargo.toml", RUST_CARGO),
            ("frontends/rust/macros/Cargo.toml", MACROS_CARGO),
            ("libhegel/Cargo.toml", C_CARGO),
        ]:
            path = self.root / rel
            path.parent.mkdir(parents=True)
            path.write_text(text)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def test_bumps_each_package_to_its_own_version(self) -> None:
        # The Rust frontend (hegeltest + macros) and libhegel version independently.
        release.apply_version_bump(self.root, "0.19.2", "0.6.0")
        for rel in release.RUST.manifests:
            text = (self.root / rel).read_text()
            self.assertIn('version = "0.19.2"', text, rel)
            self.assertNotIn('version = "0.19.1"', text, rel)
        c_text = (self.root / "libhegel" / "Cargo.toml").read_text()
        self.assertIn('version = "0.6.0"', c_text)
        self.assertNotIn('version = "0.5.0"', c_text)

    def test_bumps_internal_path_dependency_pins_per_crate(self) -> None:
        # Each internal path dep in the hegeltest manifest must follow its own
        # crate's version, or `cargo update --workspace` fails: the macros pin
        # tracks the frontend version, the hegeltest-c pin the libhegel version.
        release.apply_version_bump(self.root, "0.19.2", "0.6.0")
        rust_text = (self.root / "frontends/rust/Cargo.toml").read_text()
        self.assertIn(
            'hegeltest-macros = { version = "=0.19.2", path = "macros" }',
            rust_text,
        )
        self.assertIn(
            'hegeltest-c = { version = "=0.6.0", path = "../../libhegel", '
            "default-features = false, optional = true }",
            rust_text,
        )

    def test_leaves_external_dependencies_untouched(self) -> None:
        release.apply_version_bump(self.root, "0.19.2", "0.6.0")
        rust_text = (self.root / "frontends/rust/Cargo.toml").read_text()
        # External deps carry no `path =`, so they must be left alone — and the
        # `static-engine` feature reference is not a version pin either.
        self.assertIn('serde = { version = "1.0.103", features = ["derive"] }', rust_text)
        self.assertIn('static-engine = ["dep:hegeltest-c"]', rust_text)


class PlanReleaseTest(unittest.TestCase):
    def test_rust_only_bumps_rust_and_leaves_c(self) -> None:
        rust, c, rust_body, c_body = release.plan_release(
            "0.19.1", "0.5.0", ("patch", "This patch does a thing."), None
        )
        self.assertEqual(rust, "0.19.2")
        self.assertEqual(c, "0.5.0")
        self.assertEqual(rust_body, "This patch does a thing.")
        self.assertIsNone(c_body)

    def test_c_only_patch_bumps_both_and_auto_generates_rust_entry(self) -> None:
        rust, c, rust_body, c_body = release.plan_release(
            "0.19.1", "0.5.0", None, ("patch", "This patch tweaks the C ABI.")
        )
        self.assertEqual(rust, "0.19.2")
        self.assertEqual(c, "0.5.1")
        self.assertEqual(
            rust_body, "This release updates the `hegeltest-c` dependency to 0.5.1."
        )
        self.assertEqual(c_body, "This patch tweaks the C ABI.")

    def test_c_minor_is_only_a_rust_patch(self) -> None:
        # The whole point of independent versions: a breaking C ABI change is a
        # minor bump for libhegel but only a patch for the Rust frontend.
        rust, c, rust_body, c_body = release.plan_release(
            "0.19.1", "0.5.0", None, ("minor", "This release breaks the C ABI.")
        )
        self.assertEqual(rust, "0.19.2")
        self.assertEqual(c, "0.6.0")
        self.assertEqual(
            rust_body, "This release updates the `hegeltest-c` dependency to 0.6.0."
        )
        self.assertEqual(c_body, "This release breaks the C ABI.")

    def test_both_present_bump_independently(self) -> None:
        rust, c, rust_body, c_body = release.plan_release(
            "0.19.1",
            "0.5.0",
            ("patch", "This patch does a thing."),
            ("minor", "This release breaks the C ABI."),
        )
        self.assertEqual(rust, "0.19.2")
        self.assertEqual(c, "0.6.0")
        self.assertEqual(rust_body, "This patch does a thing.")
        self.assertEqual(c_body, "This release breaks the C ABI.")


class CurrentVersionTest(unittest.TestCase):
    def test_reads_the_package_version(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            manifest = Path(tmp) / "Cargo.toml"
            manifest.write_text(RUST_CARGO)
            self.assertEqual(release.current_version(manifest), "0.19.1")


class ComponentTest(unittest.TestCase):
    def test_tags_are_path_slash_version(self) -> None:
        self.assertEqual(release.RUST.tag("0.23.3"), "frontends/rust/0.23.3")
        self.assertEqual(release.LIBHEGEL.tag("0.24.0"), "libhegel/0.24.0")

    def test_release_files_live_in_the_component(self) -> None:
        self.assertEqual(release.RUST.release_file, "frontends/rust/RELEASE.md")
        self.assertEqual(release.LIBHEGEL.release_file, "libhegel/RELEASE.md")
        self.assertEqual(release.RUST.changelog, "frontends/rust/CHANGELOG.md")
        self.assertEqual(release.LIBHEGEL.changelog, "libhegel/CHANGELOG.md")

    def test_release_paths_cover_every_manifest_and_changelog(self) -> None:
        for component in release.COMPONENTS:
            for manifest in component.manifests:
                self.assertIn(manifest, release.RELEASE_PATHS)
            self.assertIn(component.changelog, release.RELEASE_PATHS)
        self.assertIn("Cargo.lock", release.RELEASE_PATHS)

    def test_release_paths_exist_in_the_checkout(self) -> None:
        for rel in release.RELEASE_PATHS:
            self.assertTrue((release.ROOT / rel).exists(), rel)


class ReleaseTagsTest(unittest.TestCase):
    def test_rust_only_release_tags_only_the_rust_frontend(self) -> None:
        self.assertEqual(release.release_tags("0.23.3", None), ["frontends/rust/0.23.3"])

    def test_c_release_tags_both_components(self) -> None:
        self.assertEqual(
            release.release_tags("0.23.3", "0.24.0"),
            ["frontends/rust/0.23.3", "libhegel/0.24.0"],
        )

    def test_libhegel_release_is_titled_after_libhegel(self) -> None:
        self.assertEqual(release.libhegel_release_title("0.24.0"), "libhegel v0.24.0")


class ReleasePrDetailsTest(unittest.TestCase):
    def test_rust_only_release_names_its_tag(self) -> None:
        title, body = release.release_pr_details(["frontends/rust/0.23.3"])
        self.assertEqual(title, "Release frontends/rust/0.23.3")
        self.assertIn("after tagging frontends/rust/0.23.3 failed", body)
        self.assertIn("The tags and crates.io publish succeeded.", body)

    def test_c_release_names_both_tags(self) -> None:
        title, body = release.release_pr_details(
            ["frontends/rust/0.23.3", "libhegel/0.24.0"]
        )
        self.assertEqual(title, "Release frontends/rust/0.23.3 and libhegel/0.24.0")
        self.assertIn(
            "after tagging frontends/rust/0.23.3 and libhegel/0.24.0 failed", body
        )


if __name__ == "__main__":
    unittest.main()
