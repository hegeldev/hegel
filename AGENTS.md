# AGENTS.md

The Hegel monorepo. `libhegel/` is the native property-based testing engine (the `hegeltest-c` crate, shipped as the `libhegel` C-ABI library), and `frontends/<language>/` are the language frontends that drive it through that C ABI. Each component has its own `AGENTS.md` with its specifics; this file covers what spans the repo.

## Layout

- `libhegel/` — the engine. See `libhegel/AGENTS.md`.
- `frontends/rust/` — the Rust frontend: the `hegeltest` crate plus its proc-macro crate in `macros/`. See `frontends/rust/AGENTS.md`.
- `frontends/python/` — the Python frontend: the `hegeltest` package on PyPI, a pyo3 extension (`src/`) over libhegel plus the `hegel` package (`python/hegel/`). Needs `uv`. See `frontends/python/AGENTS.md`.
- `frontends/typescript/` — the TypeScript frontend: the `@hegeldev/hegel` npm package, driving libhegel through koffi (Node, Bun, Deno) and through the raw Wasm exports (browsers). Needs Node >= 20.11. See `frontends/typescript/AGENTS.md`.
- `frontends/cpp/` — the C++ frontend: the `hegel` CMake library, driving libhegel in-process through its C ABI. Needs CMake 3.14+ and a C++20 compiler. See `frontends/cpp/AGENTS.md`.
- `Cargo.toml` — one Cargo workspace for every Rust crate in the repo (`libhegel`, `frontends/rust`, `frontends/rust/macros`, `frontends/python`); one `Cargo.lock`. The TypeScript and C++ frontends are not crates: they build `hegeltest-c` from the workspace with cargo (`just typescript::build-libhegel`; the C++ frontend's `cmake/libhegel.cmake` runs cargo itself).
- `scripts/` — lints that span the Cargo workspace. Component-specific scripts live in `<component>/scripts/`.
- `justfile` — workspace-wide recipes; `libhegel/justfile`, `frontends/rust/justfile`, `frontends/python/justfile`, `frontends/typescript/justfile` and `frontends/cpp/justfile` are `just` modules (`just libhegel::<recipe>`, `just rust::<recipe>`, `just python::<recipe>`, `just typescript::<recipe>`, `just cpp::<recipe>`) whose recipes run from their own directory.
- `.github/workflows/ci.yml` — the CI entry point. Its `changes` job works out which components a PR touches; `libhegel.yml`, `frontend-rust.yml`, `frontend-python.yml`, `frontend-typescript.yml` and `frontend-cpp.yml` are reusable per-component workflows that only run when their paths (or the shared workspace files) changed. Every Rust job starts with `.github/actions/setup-rust`; `.github/actions/build-python-wheels` builds the Python wheels for both PR checks and releases.
- `.claude/skills/` (also reachable as `.agents/skills/`) — skills shared by every component.

## Commands

```bash
just check                # definition of done for the whole repo: lint + every component's tests
just lint                 # workspace lints + each component's lints
just format
just check-coverage       # whole-workspace coverage (needs cargo-llvm-cov + llvm-tools-preview)
just libhegel::check      # one component
just rust::check
just python::check        # needs `just python::setup` once (uv)
just typescript::check    # needs `just typescript::setup` once (npm ci) and the wasm32-unknown-unknown target
just cpp::check           # needs CMake, a C++20 compiler, uv, clang-tidy, doxygen and llvm-cov
just --list libhegel      # a component's recipes
```

## Versioning and releases

- Components version independently. A release of a component at `<path>` is tagged `<path>/<version>`: `libhegel/0.47.1`, `frontends/rust/0.50.1`, `frontends/python/0.1.0`, `frontends/typescript/0.4.10`, `frontends/cpp/0.13.1`. Only libhegel tags carry a GitHub release (the prebuilt binaries); the Rust crates publish to crates.io, the Python wheels to PyPI and the npm packages (`@hegeldev/hegel` plus its five platform packages) to npm. The C++ frontend publishes nowhere: its tag is what consumers fetch (CMake `FetchContent` with `SOURCE_SUBDIR frontends/cpp`), and a release pushes its Doxygen reference to hegel.dev/cpp.
- A PR that changes `<component>/src/` (or `frontends/rust/macros/`, the Python frontend's `python/hegel/`, `Cargo.toml` or `pyproject.toml`, the TypeScript frontend's `package.json`, or the C++ frontend's `include/`, `cmake/` or `CMakeLists.txt`) needs a `<component>/RELEASE.md` — see `CONTRIBUTING.md` and the `changelog` skill. On merge, `.github/scripts/release.py` bumps the version, moves the entry into `<component>/CHANGELOG.md`, tags, and publishes.
- Every frontend ships the engine (`hegeltest` pins `hegeltest-c` exactly; the Python wheels compile libhegel in; the npm platform packages carry the libhegel binaries of the GitHub release; the C++ build compiles the in-tree libhegel it was fetched with), so a libhegel release also patch-bumps and republishes every frontend; a frontend's changelog entry is auto-generated unless the PR writes its `RELEASE.md` too. The TypeScript frontend's generated `src/libhegel-version.ts` mirrors `libhegel/Cargo.toml` (`just typescript::check-libhegel-version` enforces it; the release script moves it).
- The release automation's component list is `COMPONENTS` in `.github/scripts/release.py`; `just check-release-script` runs its tests.

## Conventions

- Tests live in `tests/`, never inline in source. Tests needing private items go in `tests/embedded/`, mirroring `src/`, wired in with `#[cfg(test)] #[path = "..."] mod tests;` from the source file (both crates, e.g. `frontends/rust/src/ffi.rs` → `frontends/rust/tests/embedded/ffi_tests.rs`).
- In tests prefer `.unwrap()` over `.expect("static message")`; use `.expect` only with a formatted value.
- Don't bind unused results to `_`; `let _ =` only to silence `#[must_use]`.
- No comments other than `// nocov` and ones with a specific function. Before committing, delete any comments you wrote.
- The engine must never panic across the FFI boundary: use the `hegel_internal_assert!` family instead of `assert!` in `frontends/rust/src/` and `libhegel/src/`, `invalid_argument!` / `EngineError::InvalidArgument` for argument validation (`scripts/check-internal-asserts.py` enforces this).
- MSRV is 1.86. Bumping it means bumping `frontends/rust/Cargo.toml`, `frontends/rust/macros/Cargo.toml`, `frontends/python/Cargo.toml`, `libhegel/Cargo.toml`, and `.github/workflows/frontend-rust.yml` together.
- The Python conventions (pytest, 100% line and branch coverage of `python/hegel`, `# nocov`) are in `frontends/python/AGENTS.md`; the Rust rules above apply to its `src/` too. The TypeScript conventions (vitest, 100% coverage, no engine mocks) are in `frontends/typescript/AGENTS.md`; the C++ conventions (GoogleTest, the `GCOVR_EXCL` coverage ratchet, clang-format/clang-tidy) in `frontends/cpp/AGENTS.md`.

## Boundaries

- Never add `// nocov` without explicit human permission. New code needs 100% line coverage; see the `coverage` skill.
- Never increase the numbers in `.github/coverage-ratchet.json` without explicit human permission.

## Skills

`new-generator`, `new-default-generator`, `add-library-support`, `new-setting`, `coverage`, `changelog` (for `RELEASE.md`), `typescript-ffi` (the TypeScript frontend's bindings to the C ABI), `self-review`. Use them when the task matches.
