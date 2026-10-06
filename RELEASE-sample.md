RELEASE_TYPE: patch

This patch makes `gs.lists()` respect `unique=True` when shrinking.

---

Every pull request which changes what users install (the Rust extension in `src/`, the Python package in `python/hegel/`, or `Cargo.toml` / `Cargo.lock`) must include a `RELEASE.md` file. This `RELEASE-sample.md` file is an example of that file. A pull request that touches only tests, docs or CI doesn't need one.

While we are on a `0.x` major version the semver levels are shifted: **breaking changes are "minor"** and **everything else (bug fixes, internal changes, and new non-breaking features / API additions) is "patch"**. So in the example above, "patch" on the first line should be replaced by "minor" only for a breaking change. "major" is reserved for the eventual 1.0 and beyond, and only maintainers should ever make a major release.

The remaining lines are the actual changelog text for this release, which should:

- concisely describe any public-facing changes, and why. Internal-only changes can be documented as e.g. "This release improves an internal invariant."
- use `single backticks` for verbatim code.

After the pull request is merged, the release job bumps the version in `Cargo.toml` (the only place it is written down: `pyproject.toml` declares it dynamic and `hegel.__version__` reads the installed metadata), and the contents of this file (except the first line) are automatically added to `CHANGELOG.md`. While the top section of `CHANGELOG.md` is still marked `unreleased`, the release ships that version instead of bumping past it. More examples can be found in `CHANGELOG.md`.
