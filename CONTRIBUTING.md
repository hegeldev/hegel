# Contributing

First: It's great that you're interested in contributing to Hegel! Thanks!

> [!NOTE]
> This guide will get fleshed out over time. In short, we welcome contributions, and will happily walk you through any complications you encounter during a contribution—just ask!

This repository holds the Hegel engine (`libhegel/`) and the language frontends built on it (`frontends/<language>/`). Hegel uses [`just`](https://github.com/casey/just) for development commands:

```bash
just test
just format
# full pre-PR check: lint + every component's tests
just check
# one component
just libhegel::check
just rust::check
just python::check   # needs uv, and `just python::setup` once
```

Of course, you can also run the underlying `cargo` commands if you prefer not to install `just`. The Python frontend needs [`uv`](https://docs.astral.sh/uv/) as well.

## Release notes

Every pull request that changes what a component ships needs a `RELEASE.md` in that component's directory: `libhegel/RELEASE.md` for a change under `libhegel/src/`, `frontends/rust/RELEASE.md` for one under `frontends/rust/src/` or `frontends/rust/macros/`, and `frontends/python/RELEASE.md` for one to the Python frontend's `src/`, `python/hegel/`, `Cargo.toml` or `pyproject.toml`. A pull request that touches only tests, docs or CI doesn't need one. CI checks this (`.github/scripts/release.py check`).

The file is a `RELEASE_TYPE` line followed by the changelog entry:

```
RELEASE_TYPE: patch

This patch makes `gs.lists()` respect `unique=True` when shrinking.
```

While every component is on a `0.x` version the semver levels are shifted: **breaking changes are `minor`**, and everything else (bug fixes, internal changes, and new non-breaking features) is `patch`. `major` is reserved for the eventual 1.0, and only maintainers make one.

The entry describes the public-facing change and why, with `single backticks` for code; an internal-only change can say "This release improves an internal invariant." The `changelog` skill (`.claude/skills/changelog/SKILL.md`) is the style guide, and each component's `CHANGELOG.md` has more examples.

The components version independently and are tagged `<path>/<version>`. Every frontend ships the engine, so a libhegel release also republishes every frontend: when a pull request releases libhegel and makes no user-facing change to a frontend, write only `libhegel/RELEASE.md` and the frontend's changelog entry is generated as an engine bump. That is a judgment call, not an automatic one: an engine fix that changes what a frontend's users see deserves an entry in that frontend's `RELEASE.md` too.

On merge, the release job bumps the component's version in its `Cargo.toml` (the Python frontend's `pyproject.toml` declares the version dynamic), moves the entry into its `CHANGELOG.md`, tags, and publishes.
