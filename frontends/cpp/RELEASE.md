RELEASE_TYPE: patch

This patch moves Hegel for C++ into the [hegeldev/hegel](https://github.com/hegeldev/hegel) monorepo, next to the engine it drives. Fetch it from there now, with the frontend as the source subdirectory:

```cmake
FetchContent_Declare(
    hegel
    GIT_REPOSITORY https://github.com/hegeldev/hegel.git
    GIT_TAG frontends/cpp/<version>
    SOURCE_SUBDIR frontends/cpp
)
```

The build no longer downloads a prebuilt libhegel: it compiles the in-tree engine with `cargo`, so a Rust toolchain is needed unless `-DHEGEL_LIBHEGEL_LIBRARY` points at a libhegel built elsewhere (`-DHEGEL_LIBHEGEL_INCLUDE_DIR` then names that library's `hegel.h`). The Nix flake is gone with the download it existed for.

This release also updates the bundled engine from 0.42.4 to 0.48.0, which changes what a C++ user can see:

- A test whose outcome or draws change when the same generated data is replayed no longer fails the run with a flaky-test error. The engine confirms each such failure by replaying it, reports it with a `note:` line saying how reliably it reproduced, and leaves out the `rerun with:` line when it could not be confirmed. The new `Settings::nondeterminism_strictness` restores the old error (`NondeterminismStrictness::Error`) or adds a one-line notice per run (`Warn`); the default is `Quiet`.
- The new `Settings::stop_on_first_failure` (default `true`) can make a run use its whole `test_cases` budget before reporting, so every distinct failure found along the way is persisted to the database.
- A run that generates no valid test case now fails with an `Unsatisfiable` error, even with the `FilterTooMuch` health check suppressed.
- Concurrent state machines no longer print a `Concurrent state machine detected` notice; a failure one of their workers finds is reported from the output the engine captured for it.
- Strings passed to the engine may contain NUL characters, `gs::from_regex()` rejects a pattern no string can match when the generator is built, integer shrinking no longer stalls short of the minimum on some sparse sets of failing values, and seeded runs may generate different values from this version on.
