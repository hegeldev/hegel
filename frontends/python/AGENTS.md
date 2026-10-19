# Python frontend

Hegel for Python: the `hegeltest` package on PyPI, imported as `hegel`. The engine in `../../libhegel` is compiled into a pyo3 extension module, `hegel._native` (`src/*.rs`, the `hegel-python` crate, a member of the root Cargo workspace); the Python package lives in `python/hegel/`, and `tests/hypothesis_port/` is Hypothesis's own test suite ported to the Hegel API (see its README for the conventions). Repo-wide conventions are in the root `AGENTS.md`.

## Commands

```bash
just python::setup                  # create .venv (uv) and build the extension (maturin develop)
just python::build                  # rebuild the extension after changing src/ or libhegel
just python::test tests/test_x.py   # run some tests (pytest arguments pass through)
just python::format                 # ruff format + fix (cargo fmt runs from the root justfile)
just python::check                  # everything CI checks for this frontend
```

`just python::check` runs `check-lint` (ruff, mypy), `check-coverage` and `check-hypothesis-port`. `cargo fmt`, `cargo clippy` and `just check-release-script` cover the Rust half from the root. Plain cargo commands run from this directory use the interpreter in `.venv` (see `.cargo/config.toml`), so run `just python::setup` first.

## Rules

- **Coverage is 100%.** `just python::check-coverage` requires full line and branch coverage of `python/hegel` from the package's own tests (`tests/`, without the ported suite). Cover new code with tests; mark genuinely unreachable lines `# nocov` rather than lowering the bar. Code that only some Python versions reach is marked `# unreachable on py3.X` instead, which excludes it only when `check-coverage` runs on 3.X.
- **Engine divergences are not worked around.** When libhegel behaves differently from what Python's `re` or Hypothesis would do, open an issue against this repository and mark the affected test `xfail` (strict, so it flags when the engine is fixed), instead of papering over it in the bindings. The fix belongs in the engine, which lives in this repository: fix it in `../../libhegel` (with a `libhegel/RELEASE.md`) and remove the xfail in the same PR, closing the issue.
- **Every PR that changes what ships needs a `RELEASE.md` here.** That means any change to `src/`, `python/hegel/`, `Cargo.toml` or `pyproject.toml`. See `RELEASE-sample.md` for the format and the semver rules (on 0.x, breaking changes are `minor`, everything else `patch`). CI checks this (`.github/scripts/release.py check`); on merge to main the release job bumps the version, moves the text into `CHANGELOG.md`, tags `frontends/python/<version>`, builds wheels for Linux (x86-64, ARM64), Apple-silicon macOS and Windows plus an sdist, and publishes them to PyPI. A libhegel release re-releases this frontend too, with an auto-generated engine-bump entry unless the PR writes a `RELEASE.md` here.
- **The version lives in `Cargo.toml` only.** `pyproject.toml` declares it dynamic (maturin reads Cargo.toml) and `hegel.__version__` reads the installed metadata. Don't edit it by hand; the release job does.
- **The engine is always the in-tree libhegel.** There is no pin to bump: `Cargo.toml` depends on `../../libhegel` by path, and the sdist bundles it.

## Gotchas

- The crate has no Rust tests and is excluded from the Rust coverage and minimal-versions runs; its tests are the pytest suite, which exercises every binding.
- `_native.pyi` is the type stub for the extension; keep it in step with `src/` or mypy's view of the package drifts.
- The extension is built with `pyo3/extension-module` only through maturin (`pyproject.toml`); plain `cargo test`/`cargo clippy` link against the interpreter instead, which is why `cargo test --workspace --all-features` excludes this crate.
