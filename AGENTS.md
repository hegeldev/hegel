# AGENTS.md

Guidance for coding agents (and people) working in this repository.

hegel-python is the Python binding of the Hegel property-based testing engine.
The engine is libhegel, built from source from the `hegeltest-c` crate (pinned
exactly in `Cargo.toml`) into a pyo3 extension module, `hegel._native`
(`src/*.rs`). The Python package lives in `python/hegel/`; `python/hegel/hypothesis/`
is a compatibility layer reimplementing Hypothesis's API on top of it, and
`tests/hypothesis_port/` is Hypothesis's own test suite ported to run against
that layer.

## Workflow

```bash
just setup                  # create .venv (uv) and build the extension (maturin develop)
just build                  # rebuild the extension after changing src/
just test tests/test_x.py   # run some tests (pytest arguments pass through)
just format                 # ruff format + fix, cargo fmt
just check                  # everything CI checks; must pass before you commit
```

`just check` runs `check-format`, `check-lint` (ruff, clippy), `check-types`
(mypy), `check-release-script`, `check-coverage` and `check-hypothesis-port`.
Plain cargo commands use the interpreter in `.venv` (see `.cargo/config.toml`),
so run `just setup` first.

## Rules

- **Coverage is 100%.** `just check-coverage` requires full line and branch
  coverage of `python/hegel`, excluding the Hypothesis compatibility layer
  (`python/hegel/hypothesis/`). Cover new code with tests; mark genuinely
  unreachable lines `# nocov` rather than lowering the bar.
- **Engine divergences are not worked around.** When libhegel behaves
  differently from what Python's `re` or Hypothesis would do, record it in
  `docs/engine-issues.md` and mark the affected test `xfail` (strict, so it
  flags when the engine is fixed), instead of papering over it in the
  bindings. The fix belongs in the engine.
- **Every PR that changes what ships needs a `RELEASE.md`.** That means any
  change to `src/`, `python/hegel/`, `Cargo.toml` or `Cargo.lock`. See
  `RELEASE-sample.md` for the format and the semver rules (on 0.x, breaking
  changes are `minor`, everything else `patch`). CI checks this
  (`.github/scripts/release.py check`); on merge to main the release job bumps
  the version, moves the text into `CHANGELOG.md`, tags and cuts a GitHub
  release.
- **The version lives in `Cargo.toml` only.** `pyproject.toml` declares it
  dynamic (maturin reads Cargo.toml) and `hegel.__version__` reads the
  installed metadata. Don't edit it by hand; the release job does.
- **Engine bumps** go through `.github/workflows/bump-hegel-rust.yml`, which
  rewrites the `hegeltest-c` pin, updates `Cargo.lock`, and opens a PR. To do
  it by hand: change the `=X.Y.Z` pin, run `cargo update -p hegeltest-c`, and
  update the version named at the top of `docs/engine-issues.md`.
