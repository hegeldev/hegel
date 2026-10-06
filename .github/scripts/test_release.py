#!/usr/bin/env python3
"""Tests for the file-rewriting half of release.py.

The git/gh half of `release()` can only run in CI with real credentials, but
everything it does to the working tree goes through `prepare_release`, which
these tests run against scratch copies of the repository's real manifests.
"""

from __future__ import annotations

import shutil
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import release  # noqa: E402

ROOT = release.ROOT


def write_release(root: Path, release_type: str, text: str) -> None:
    (root / "RELEASE.md").write_text(f"RELEASE_TYPE: {release_type}\n\n{text}\n")


class TestVersions(unittest.TestCase):
    def test_bump(self) -> None:
        self.assertEqual(release.bump_version("0.1.2", "patch"), "0.1.3")
        self.assertEqual(release.bump_version("0.1.2", "minor"), "0.2.0")
        self.assertEqual(release.bump_version("0.1.2", "major"), "1.0.0")

    def test_reads_the_real_manifest(self) -> None:
        version = release.current_version(ROOT / "Cargo.toml")
        self.assertRegex(version, r"^\d+\.\d+\.\d+$")

    def test_set_version_only_touches_the_package(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            cargo = Path(d) / "Cargo.toml"
            shutil.copy(ROOT / "Cargo.toml", cargo)
            before = cargo.read_text()
            release.set_version(cargo, "9.8.7")
            self.assertEqual(release.current_version(cargo), "9.8.7")
            after = cargo.read_text()
            # Only the package version line differs; dependency pins such as
            # hegeltest-c's are untouched.
            changed = [
                (a, b)
                for a, b in zip(before.splitlines(), after.splitlines())
                if a != b
            ]
            self.assertEqual(len(changed), 1)
            self.assertEqual(changed[0][1], 'version = "9.8.7"')

    def test_unreleased_section_is_released_as_is(self) -> None:
        self.assertEqual(release.plan_version("0.1.0", "minor", "0.1.0"), "0.1.0")
        self.assertEqual(release.plan_version("0.1.0", "minor", None), "0.2.0")
        with self.assertRaises(ValueError):
            release.plan_version("0.1.1", "patch", "0.1.0")


class TestReleaseFile(unittest.TestCase):
    def test_parse(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            write_release(Path(d), "patch", "Fixes a thing.")
            self.assertEqual(
                release.parse_release_file(Path(d) / "RELEASE.md"),
                ("patch", "Fixes a thing."),
            )

    def test_rejects_bad_type_and_empty_text(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "RELEASE.md"
            path.write_text("RELEASE_TYPE: tiny\n\nText.\n")
            with self.assertRaises(ValueError):
                release.parse_release_file(path)
            path.write_text("RELEASE_TYPE: patch\n\n\n")
            with self.assertRaises(ValueError):
                release.parse_release_file(path)

    def test_sample_is_valid(self) -> None:
        release.parse_release_file(ROOT / "RELEASE-sample.md")


class TestPrepareRelease(unittest.TestCase):
    def scratch_repo(self, d: str) -> Path:
        root = Path(d)
        for name in ["Cargo.toml", "Cargo.lock"]:
            shutil.copy(ROOT / name, root / name)
        shutil.copytree(ROOT / "src", root / "src")
        return root

    def test_releases_an_unreleased_version_without_bumping(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            root = self.scratch_repo(d)
            version = release.current_version(root / "Cargo.toml")
            (root / "CHANGELOG.md").write_text(
                f"# Changelog\n\n## {version} - unreleased\n\nFirst release.\n"
            )
            write_release(root, "patch", "Also this.")

            new, content = release.prepare_release(root)

            self.assertEqual(new, version)
            self.assertEqual(content, "Also this.")
            changelog = (root / "CHANGELOG.md").read_text()
            self.assertNotIn("unreleased", changelog)
            self.assertRegex(
                changelog,
                rf"^# Changelog\n\n## {version} - \d{{4}}-\d\d-\d\d\n\n"
                r"First release\.\n\nAlso this\.\n$",
            )

    def test_bumps_cargo_toml_lock_and_changelog(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            root = self.scratch_repo(d)
            release.set_version(root / "Cargo.toml", "0.3.4")
            (root / "CHANGELOG.md").write_text(
                "# Changelog\n\n## 0.3.4 - 2026-01-01\n\nOld.\n"
            )
            write_release(root, "minor", "Breaks a thing.")

            new, _ = release.prepare_release(root)

            self.assertEqual(new, "0.4.0")
            self.assertEqual(release.current_version(root / "Cargo.toml"), "0.4.0")
            self.assertIn(
                'name = "hegel-python"\nversion = "0.4.0"',
                (root / "Cargo.lock").read_text(),
            )
            changelog = (root / "CHANGELOG.md").read_text()
            self.assertRegex(
                changelog,
                r"^# Changelog\n\n## 0\.4\.0 - \d{4}-\d\d-\d\d\n\n"
                r"Breaks a thing\.\n\n## 0\.3\.4 - 2026-01-01\n\nOld\.\n$",
            )

    def test_the_real_changelog_is_releasable(self) -> None:
        release.plan_version(
            release.current_version(ROOT / "Cargo.toml"),
            "patch",
            release.unreleased_version(ROOT / "CHANGELOG.md"),
        )


if __name__ == "__main__":
    unittest.main()
