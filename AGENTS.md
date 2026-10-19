# AGENTS.md

The Hegel monorepo. `libhegel/` is the native property-based testing engine (the `hegeltest-c` crate, shipped as the `libhegel` C-ABI library), and `frontends/<language>/` are the language frontends that drive it through that C ABI. Each component has its own `AGENTS.md` with its specifics; this file covers what spans the repo.

## Layout

- `libhegel/` — the engine. See `libhegel/AGENTS.md`.
- `frontends/rust/` — the Rust frontend: the `hegeltest` crate plus its proc-macro crate in `macros/`. See `frontends/rust/AGENTS.md`.
- `Cargo.toml` — one Cargo workspace for every Rust crate in the repo (`libhegel`, `frontends/rust`, `frontends/rust/macros`); one `Cargo.lock`. Frontends in other languages ignore it.
- `scripts/` — lints that span the Cargo workspace. Component-specific scripts live in `<component>/scripts/`.
- `justfile` — workspace-wide recipes; `libhegel/justfile` and `frontends/rust/justfile` are `just` modules (`just libhegel::<recipe>`, `just rust::<recipe>`) whose recipes run from their own directory.
- `.github/workflows/ci.yml` — the CI entry point. Its `changes` job works out which components a PR touches; `libhegel.yml` and `frontend-rust.yml` are reusable per-component workflows that only run when their paths (or the shared workspace files) changed. Every Rust job starts with `.github/actions/setup-rust`.
- `.claude/skills/` (also reachable as `.agents/skills/`) — skills shared by every component.

## Commands

```bash
just check                # definition of done for the whole repo: lint + every component's tests
just lint                 # workspace lints + each component's lints
just format
just check-coverage       # whole-workspace coverage (needs cargo-llvm-cov + llvm-tools-preview)
just libhegel::check      # one component
just rust::check
just --list libhegel      # a component's recipes
```

## Versioning and releases

- Components version independently. A release of a component at `<path>` is tagged `<path>/<version>`: `libhegel/0.47.1`, `frontends/rust/0.50.1`. Only libhegel tags carry a GitHub release (the prebuilt binaries); crates publish to crates.io.
- A PR that changes `<component>/src/` (or `frontends/rust/macros/`) needs a `<component>/RELEASE.md` — see `frontends/rust/RELEASE-sample.md` and the `changelog` skill. On merge, `.github/scripts/release.py` bumps the version, moves the entry into `<component>/CHANGELOG.md`, tags, and publishes.
- `hegeltest` pins `hegeltest-c` exactly, so a libhegel release also patch-bumps and republishes the Rust frontend; its changelog entry is auto-generated unless the PR writes a `frontends/rust/RELEASE.md` too.
- The release automation's component list is `COMPONENTS` in `.github/scripts/release.py`; `just check-release-script` runs its tests.

## Conventions

- Tests live in `tests/`, never inline in source. Tests needing private items go in `tests/embedded/`, mirroring `src/`, wired in with `#[cfg(test)] #[path = "..."] mod tests;` from the source file (both crates, e.g. `frontends/rust/src/ffi.rs` → `frontends/rust/tests/embedded/ffi_tests.rs`).
- In tests prefer `.unwrap()` over `.expect("static message")`; use `.expect` only with a formatted value.
- Don't bind unused results to `_`; `let _ =` only to silence `#[must_use]`.
- No comments other than `// nocov` and ones with a specific function. Before committing, delete any comments you wrote.
- The engine must never panic across the FFI boundary: use the `hegel_internal_assert!` family instead of `assert!` in `frontends/rust/src/` and `libhegel/src/`, `invalid_argument!` / `EngineError::InvalidArgument` for argument validation (`scripts/check-internal-asserts.py` enforces this).
- MSRV is 1.86. Bumping it means bumping `frontends/rust/Cargo.toml`, `frontends/rust/macros/Cargo.toml`, `libhegel/Cargo.toml`, and `.github/workflows/frontend-rust.yml` together.

## Boundaries

- Never add `// nocov` without explicit human permission. New code needs 100% line coverage; see the `coverage` skill.
- Never increase the numbers in `.github/coverage-ratchet.json` without explicit human permission.

## Skills

`new-generator`, `new-default-generator`, `add-library-support`, `new-setting`, `coverage`, `changelog` (for `RELEASE.md`), `self-review`. Use them when the task matches.
