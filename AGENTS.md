# AGENTS.md

The Hegel monorepo. `libhegel/` is the native property-based testing engine (the `hegeltest-c` crate, shipped as the `libhegel` C-ABI library), and `frontends/<language>/` are the language frontends that drive it through that C ABI. Each component has its own `AGENTS.md` with its specifics; this file covers what spans the repo.

## Layout

- `libhegel/` — the engine. See `libhegel/AGENTS.md`.
- `frontends/rust/` — the Rust frontend: the `hegeltest` crate plus its proc-macro crate in `macros/`. See `frontends/rust/AGENTS.md`.
- `frontends/python/` — the Python frontend: the `hegeltest` package on PyPI, a pyo3 extension (`src/`) over libhegel plus the `hegel` package (`python/hegel/`). Needs `uv`. See `frontends/python/AGENTS.md`.
- `frontends/typescript/` — the TypeScript frontend: the `@hegeldev/hegel` npm package, driving libhegel through koffi (Node, Bun, Deno) and through the raw Wasm exports (browsers). Needs Node >= 20.11. See `frontends/typescript/AGENTS.md`.
- `frontends/cpp/` — the C++ frontend: the `hegel` CMake library, driving libhegel in-process through its C ABI. Needs CMake 3.14+ and a C++20 compiler. See `frontends/cpp/AGENTS.md`.
- `frontends/java/` — the Java frontend: the `dev.hegel:hegel` (FFM, Java 22+), `dev.hegel:hegel-jna` (JNA, Java 17+) and `dev.hegel:hegel-lowlevel` Maven artifacts, one multi-module Maven build. Needs a JDK (22+ for the whole reactor) and Maven. See `frontends/java/AGENTS.md`.
- `frontends/csharp/` — the .NET frontend: the `Hegel` NuGet package (`src/Hegel/`, C#), driving libhegel in-process through its C ABI with source-generated P/Invoke, and `Hegel.FSharp` (`src/Hegel.FSharp/`), the F# surface over it. Needs the .NET 10 SDK (the libraries target .NET 8). See `frontends/csharp/AGENTS.md`.
- `frontends/ocaml/` — the OCaml frontend: the `hegel` opam package (plus `ppx_hegel_compat`, `ppx_hegel_generator`, `ppx_hegel_test`), driving libhegel in-process through ctypes. Needs opam (OCaml >= 5.1). See `frontends/ocaml/AGENTS.md`.
- `Cargo.toml` — one Cargo workspace for every Rust crate in the repo (`libhegel`, `frontends/rust`, `frontends/rust/macros`, `frontends/python`); one `Cargo.lock`. The TypeScript, C++, Java, C# and OCaml frontends are not crates: they build `hegeltest-c` from the workspace with cargo (`just typescript::build-libhegel`, `just java::build-libhegel`, `just csharp::build-libhegel`, `just ocaml::build-libhegel`; the C++ frontend's `cmake/libhegel.cmake` runs cargo itself).
- `scripts/` — lints that span the Cargo workspace. Component-specific scripts live in `<component>/scripts/`.
- `justfile` — workspace-wide recipes; `libhegel/justfile`, `frontends/rust/justfile`, `frontends/python/justfile`, `frontends/typescript/justfile`, `frontends/cpp/justfile`, `frontends/java/justfile`, `frontends/csharp/justfile` and `frontends/ocaml/justfile` are `just` modules (`just libhegel::<recipe>`, `just rust::<recipe>`, `just python::<recipe>`, `just typescript::<recipe>`, `just cpp::<recipe>`, `just java::<recipe>`, `just csharp::<recipe>`, `just ocaml::<recipe>`) whose recipes run from their own directory.
- `.github/workflows/ci.yml` — the CI entry point. Its `changes` job works out which components a PR touches; `libhegel.yml`, `frontend-rust.yml`, `frontend-python.yml`, `frontend-typescript.yml`, `frontend-cpp.yml`, `frontend-java.yml`, `frontend-csharp.yml` and `frontend-ocaml.yml` are reusable per-component workflows that only run when their paths (or the shared workspace files) changed. Every Rust job starts with `.github/actions/setup-rust`; `.github/actions/build-python-wheels` builds the Python wheels for both PR checks and releases; every OCaml job starts with `.github/actions/setup-opam` (`.github/ocaml/` holds its CI-only opam and dune files).
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
just java::check          # needs a JDK 22+ and Maven
just csharp::check        # needs the .NET 10 SDK
just ocaml::check         # needs `just ocaml::setup` once (opam, in a switch with OCaml >= 5.1)
just --list libhegel      # a component's recipes
```

## Versioning and releases

- Components version independently. A release of a component at `<path>` is tagged `<path>/<version>`: `libhegel/0.47.1`, `frontends/rust/0.50.1`, `frontends/python/0.1.0`, `frontends/typescript/0.4.10`, `frontends/cpp/0.13.1`, `frontends/java/0.10.2`, `frontends/csharp/0.2.0`, `frontends/ocaml/0.26.4`. libhegel tags carry a GitHub release (the prebuilt binaries), and OCaml tags one holding the opam source tarball (the frontend plus the bundled libhegel binaries); the Rust crates publish to crates.io, the Python wheels to PyPI, the npm packages (`@hegeldev/hegel` plus its five platform packages) to npm, the Maven artifacts (`dev.hegel:hegel`, `dev.hegel:hegel-jna`, `dev.hegel:hegel-lowlevel`) to Maven Central, the `Hegel`, `Hegel.FSharp` and `Hegel.Xunit` packages to NuGet, and the opam packages to opam-repository through the manual `ocaml-opam-release.yml` workflow. The C++ frontend publishes nowhere: its tag is what consumers fetch (CMake `FetchContent` with `SOURCE_SUBDIR frontends/cpp`), and a release pushes its Doxygen reference to hegel.dev/cpp.
- A PR that changes `<component>/src/` (or `frontends/rust/macros/`, the Python frontend's `python/hegel/`, `Cargo.toml` or `pyproject.toml`, the TypeScript frontend's `package.json`, the C++ frontend's `include/`, `cmake/` or `CMakeLists.txt`, the Java frontend's `<module>/src/main/` or `pom.xml` files, the C# frontend's `Directory.Build.props`, or the OCaml frontend's `lib/`, `ppx/`, `template/` or `dune-project`) needs a `<component>/RELEASE/<slug>.md`, one file per PR, named after the change — see `CONTRIBUTING.md` and the `changelog` skill. The old single `<component>/RELEASE.md` is refused. On merge, `.github/scripts/release.py` bumps the version, moves the entry into `<component>/CHANGELOG.md`, tags, and publishes.
- Every frontend ships the engine (`hegeltest` pins `hegeltest-c` exactly; the Python wheels compile libhegel in; the npm platform packages, the Java jars, the NuGet package and the opam tarball carry the libhegel binaries of the GitHub release; the C++ build compiles the in-tree libhegel it was fetched with), so a libhegel release also patch-bumps and republishes every frontend; a frontend's changelog entry is auto-generated unless the PR writes a release file for it too. The TypeScript frontend's generated `src/libhegel-version.ts` mirrors `libhegel/Cargo.toml` (`just typescript::check-libhegel-version` enforces it; the release script moves it). The OCaml frontend's version lives in `dune-project` and the generated `*.opam` files (`just ocaml::check-opam` enforces that they match).
- The release automation's component list is `COMPONENTS` in `.github/scripts/release.py`; `just check-release-script` runs its tests.

## Conventions

- Tests live in `tests/`, never inline in source. Tests needing private items go in `tests/embedded/`, mirroring `src/`, wired in with `#[cfg(test)] #[path = "..."] mod tests;` from the source file (both crates, e.g. `frontends/rust/src/ffi.rs` → `frontends/rust/tests/embedded/ffi_tests.rs`).
- In tests prefer `.unwrap()` over `.expect("static message")`; use `.expect` only with a formatted value.
- Don't bind unused results to `_`; `let _ =` only to silence `#[must_use]`.
- No comments other than `// nocov` and ones with a specific function. Before committing, delete any comments you wrote.
- The engine must never panic across the FFI boundary: use the `hegel_internal_assert!` family instead of `assert!` in `frontends/rust/src/` and `libhegel/src/`, `invalid_argument!` / `EngineError::InvalidArgument` for argument validation (`scripts/check-internal-asserts.py` enforces this).
- MSRV is 1.86. Bumping it means bumping `frontends/rust/Cargo.toml`, `frontends/rust/macros/Cargo.toml`, `frontends/python/Cargo.toml`, `libhegel/Cargo.toml`, and `.github/workflows/frontend-rust.yml` together.
- The Python conventions (pytest, 100% line and branch coverage of `python/hegel`, `# nocov`) are in `frontends/python/AGENTS.md`; the Rust rules above apply to its `src/` too. The TypeScript conventions (vitest, 100% coverage, no engine mocks) are in `frontends/typescript/AGENTS.md`; the C++ conventions (GoogleTest, the `GCOVR_EXCL` coverage ratchet, clang-format/clang-tidy) in `frontends/cpp/AGENTS.md`, the Java ones (JUnit 5, 100% JaCoCo instruction and branch coverage per module, palantir-java-format) in `frontends/java/AGENTS.md`, the C# and F# ones (xunit, 100% coverlet line coverage per package, `dotnet format` and fantomas) in `frontends/csharp/AGENTS.md`, and the OCaml ones (Alcotest, bisect_ppx at 100%, ocamlformat) in `frontends/ocaml/AGENTS.md`.

## Boundaries

- Never add `// nocov` without explicit human permission. New code needs 100% line coverage; see the `coverage` skill.
- Never increase the numbers in `.github/coverage-ratchet.json` without explicit human permission.

## Skills

`new-generator`, `new-default-generator`, `add-library-support`, `new-setting`, `coverage`, `changelog` (for `RELEASE/<slug>.md`), `typescript-ffi` (the TypeScript frontend's bindings to the C ABI), `self-review`. Use them when the task matches.
