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
just typescript::check   # needs Node, and `just typescript::setup` once
just cpp::check   # needs CMake and a C++20 compiler
just java::check   # needs a JDK (22+) and Maven
just csharp::check   # needs the .NET 10 SDK
just ocaml::check   # needs opam, and `just ocaml::setup` once
```

Of course, you can also run the underlying `cargo` commands if you prefer not to install `just`. The Python frontend needs [`uv`](https://docs.astral.sh/uv/) as well, and the TypeScript frontend needs [Node](https://nodejs.org/) (20.11 or later) plus the `wasm32-unknown-unknown` Rust target (`rustup target add wasm32-unknown-unknown`) to build the engine for browsers. The C++ frontend needs [CMake](https://cmake.org/) 3.14+ and a C++20 compiler (its lints, docs and coverage also want `uv`, `clang-tidy`, `doxygen` and `llvm-cov`; see `frontends/cpp/AGENTS.md`). The Java frontend needs a JDK (22 or later for the whole build; 17 suffices for `hegel-jna`) and [Maven](https://maven.apache.org/). The C# frontend needs the [.NET SDK](https://dotnet.microsoft.com/download) (10, with the .NET 8 runtime to also test the library's own target framework). The OCaml frontend needs [opam](https://opam.ocaml.org/) with a switch of OCaml 5.1 or later.

## Release notes

Every pull request that changes what a component ships needs a `RELEASE.md` in that component's directory: `libhegel/RELEASE.md` for a change under `libhegel/src/`, `frontends/rust/RELEASE.md` for one under `frontends/rust/src/` or `frontends/rust/macros/`, `frontends/python/RELEASE.md` for one to the Python frontend's `src/`, `python/hegel/`, `Cargo.toml` or `pyproject.toml`, `frontends/typescript/RELEASE.md` for one to the TypeScript frontend's `src/` or `package.json`, `frontends/cpp/RELEASE.md` for one to the C++ frontend's `src/`, `include/`, `cmake/` or `CMakeLists.txt`, `frontends/java/RELEASE.md` for one to the Java frontend's `shared/src/main/`, a module's `src/main/` or any of its `pom.xml` files, `frontends/csharp/RELEASE.md` for one to the C# frontend's `src/` or `Directory.Build.props`, and `frontends/ocaml/RELEASE.md` for one to the OCaml frontend's `lib/`, `ppx/`, `template/` or `dune-project`. A pull request that touches only tests, docs or CI doesn't need one. CI checks this (`.github/scripts/release.py check`).

The file is a `RELEASE_TYPE` line followed by the changelog entry:

```
RELEASE_TYPE: patch

This patch makes `gs.lists()` respect `unique=True` when shrinking.
```

While every component is on a `0.x` version the semver levels are shifted: **breaking changes are `minor`**, and everything else (bug fixes, internal changes, and new non-breaking features) is `patch`. `major` is reserved for the eventual 1.0, and only maintainers make one.

The entry describes the public-facing change and why, with `single backticks` for code; an internal-only change can say "This release improves an internal invariant." The `changelog` skill (`.claude/skills/changelog/SKILL.md`) is the style guide, and each component's `CHANGELOG.md` has more examples.

The components version independently and are tagged `<path>/<version>`. Every frontend ships the engine, so a libhegel release also republishes every frontend: when a pull request releases libhegel and makes no user-facing change to a frontend, write only `libhegel/RELEASE.md` and the frontend's changelog entry is generated as an engine bump. That is a judgment call, not an automatic one: an engine fix that changes what a frontend's users see deserves an entry in that frontend's `RELEASE.md` too.

On merge, the release job bumps the component's version in its `Cargo.toml` (the Python frontend's `pyproject.toml` declares the version dynamic; the TypeScript frontend's version is in its `package.json` and lockfile; the C++ frontend's is the `project()` version in its `CMakeLists.txt`; the Java frontend's in its parent `pom.xml`, repeated by each module pom's `<parent>`; the C# frontend's (both packages) in its `Directory.Build.props`; the OCaml frontend's in its `dune-project` and the `*.opam` files generated from it), moves the entry into its `CHANGELOG.md`, tags, and publishes.
