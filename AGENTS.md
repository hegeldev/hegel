# AGENTS.md

Hegel for Rust: the `hegeltest` frontend (root crate) drives the `hegel-c` engine (built as the `libhegel` C-ABI library) in-process through the same C ABI as every other language binding. `hegel-macros/` is the proc-macro crate.

## Commands

```bash
just check                # definition of done: lint + tests + all-features tests
just test                 # cargo test
just lint                 # clippy, rustfmt --check, and the scripts/check-*.py lints
just format
just check-coverage       # needs cargo-llvm-cov + llvm-tools-preview
just c-test               # hegel-c smoke tests + example C programs
just miri                 # fast core of the suite under Miri (nightly + miri component)
just c-header             # regenerate hegel-c/include/hegel.h and src/ffi/sys/fns.rs
just check-lark           # parse engine-generated strings from the Lark fixture grammars with Lark itself (needs uv)
just check-parse          # compare the engine's grammar parser with Lark's on generated strings and their mutants (needs uv)
just lark-fixtures        # regenerate the Lark fixtures' .expected files from Lark (needs uv)
cargo test test_name      # one test
```

## Gotchas

- `hegel-c/include/hegel.h` and `src/ffi/sys/fns.rs` are generated from `hegel-c/src/lib.rs`; never edit them by hand, run `just c-header` after changing an exported `hegel_*` function.
- MSRV is 1.86. Bumping it means bumping `Cargo.toml`, `hegel-macros/Cargo.toml`, `hegel-c/Cargo.toml`, and `ci.yml` together.
- The engine must never panic across the FFI boundary: use the `hegel_internal_assert!` family instead of `assert!` in `src/` and `hegel-c/src/`, `invalid_argument!` / `EngineError::InvalidArgument` for argument validation (`scripts/check-internal-asserts.py` enforces this).
- Generators never open a span around their own body (`TestCase` does), and composites draw components through `tc.draw_silent` / `tc.draw_and_print`, never a component's `do_draw`. Span labels are opaque `u64`s; there is no enumeration of them anywhere and you must not add one. Details in the `new-generator` skill.
- A new setting is not done until it is reachable every way the others are (builder, `hegel.toml`, C ABI, `HEGEL_*` env var, docs); follow the `new-setting` skill.
- Nondeterministic-test handling in the engine is documented in `hegel-c/docs/nondeterminism.md`; read it before touching `counterexample.rs`, `graph*.rs`, `nd/`, or `core/replay.rs`.
- Grammar strings (`gs::from_lark`, `hegel-c/src/native/lark/`) are mapped in `hegel-c/docs/grammar-strings.md`: a string is one grammar-constrained choice, checked on replay by the Earley parser in `lark/parse.rs` and shrunk through its derivation in `shrinker/grammar.rs`; the module docs of `lark/generate.rs` describe the generator's sizing and lexability design. A grammar or pattern is user input that can make the engine build without bound, so probe such code under a memory cap.
- The `__` feature prefix marks a feature as internal; coverage enables every other root-crate feature automatically.

## Conventions

- Tests live in `tests/`, never inline in source. Tests needing private items go in `tests/embedded/`, mirroring `src/`, wired in with `#[cfg(test)] #[path = "..."] mod tests;` from the source file (both crates, e.g. `src/ffi.rs` → `tests/embedded/ffi_tests.rs`).
- Import generators as `use hegel::generators as gs;` and call them as `gs::booleans()`, including inside string-literal code snippets. `gs::booleans()` is the default throwaway generator.
- In tests prefer `.unwrap()` over `.expect("static message")`; use `.expect` only with a formatted value.
- Don't bind unused results to `_`; `let _ =` only to silence `#[must_use]`.
- No comments other than `// nocov` and ones with a specific function. Before committing, delete any comments you wrote.

## Boundaries

- Never add `// nocov` without explicit human permission. New code needs 100% line coverage; see the `coverage` skill.
- Never increase the numbers in `.github/coverage-ratchet.json` without explicit human permission.

## Skills

`new-generator`, `new-default-generator`, `add-library-support`, `new-setting`, `coverage`, `changelog` (for `RELEASE.md`), `self-review`. Use them when the task matches.
