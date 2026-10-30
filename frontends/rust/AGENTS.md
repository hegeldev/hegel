# Rust frontend

Hegel for Rust: the `hegeltest` crate (lib name `hegel`) drives the engine in `../../libhegel` in-process through its C ABI, exactly like every other language frontend. `macros/` is the `hegeltest-macros` proc-macro crate. Repo-wide conventions are in the root `AGENTS.md`.

## Commands

```bash
just rust::check              # lint + tests + all-features tests
just rust::test               # cargo test for hegeltest + hegeltest-macros
just rust::check-docs         # cargo +nightly docs-rs
just rust::miri               # fast core of the suite under Miri (nightly + miri component)
cargo test -p hegeltest test_name
```

## Gotchas

- `src/ffi/sys/fns.rs` is generated from `libhegel/src/lib.rs`; never edit it by hand, run `just libhegel::header`.
- By default `build.rs` builds the engine from `../../libhegel` as a shared library the frontend loads at runtime; the `static-engine` feature links it in as a Rust dependency instead. `HEGEL_C_LIB_DIR` points the loader at a prebuilt library.
- Generators never open a span around their own body (`TestCase` does), and composites draw components through `tc.draw_silent` / `tc.draw_and_print`, never a component's `do_draw`. Details in the `new-generator` skill.
- A new setting is not done until it is reachable every way the others are (builder, `hegel.toml`, C ABI, `HEGEL_*` env var, docs); follow the `new-setting` skill.
- The `__` feature prefix marks a feature as internal; coverage enables every other feature automatically.
- `examples/sqlite/` is a standalone package with its own `[workspace]` table, so the repo's workspace, lockfile and `--workspace` commands never touch it (it compiles SQLite). `just rust::check-sqlite` is its lint, test and a run of its binary; add other heavy-dependency examples the same way rather than as `examples/*.rs`.
- Import generators as `use hegel::generators as gs;` and call them as `gs::booleans()`, including inside string-literal code snippets. `gs::booleans()` is the default throwaway generator (`scripts/check-generator-imports.py` enforces the import form in tests).
