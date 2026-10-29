# libhegel

The native engine behind every Hegel frontend: generation, shrinking, the example database, and the decision of what to run next, exposed through a C ABI (`src/lib.rs`, header `include/hegel.h`). It is the `hegeltest-c` crate; `#![no_std]` by default with the `std` feature providing the runtime. Repo-wide conventions are in the root `AGENTS.md`.

## Commands

```bash
just libhegel::check          # lint + tests
just libhegel::test           # cargo test -p hegeltest-c
just libhegel::c-test         # smoke tests (dlopen) + example C programs
just libhegel::header         # regenerate include/hegel.h and the Rust frontend's src/ffi/sys/fns.rs
just libhegel::miri           # C-ABI lifecycle tests under Miri (nightly + miri component)
just libhegel::check-lark     # parse engine-generated strings with Lark itself (needs uv)
just libhegel::check-parse    # compare the engine's grammar parser with Lark's (needs uv)
just libhegel::lark-fixtures  # regenerate the Lark fixtures' .expected files (needs uv)
just libhegel::unicodedata    # regenerate the Unicode tables and per-version deltas (downloads older UCD files; needs uv)
cargo test -p hegeltest-c test_name
```

## Gotchas

- `include/hegel.h` and `frontends/rust/src/ffi/sys/fns.rs` are generated from `src/lib.rs`; never edit them by hand, run `just libhegel::header` after changing an exported `hegel_*` function.
- Every frontend talks to the engine only through the `hegel_*` functions; a C ABI change is a libhegel release (a `RELEASE/<slug>.md` here) and usually needs matching changes in `frontends/`.
- Never panic across the FFI boundary: `hegel_internal_assert!` returns an `InternalError`, `invalid_argument!` / `EngineError::InvalidArgument` rejects bad arguments. `src/control.rs` must not contain a `panic!`.
- Nondeterministic-test handling is documented in `docs/nondeterminism.md`; read it before touching `counterexample.rs`, `graph*.rs`, `nd/`, or `core/replay.rs`.
- Grammar strings (`src/native/lark/`) are mapped in `docs/grammar-strings.md`: a string is one grammar-constrained choice, checked on replay by the Earley parser in `lark/parse.rs` and shrunk through its derivation in `shrinker/grammar.rs`; the module docs of `lark/generate.rs` describe the generator's sizing and lexability design. A grammar or pattern is user input that can make the engine build without bound, so probe such code under a memory cap.
- Span labels are opaque `u64`s (`src/native/labels.rs`); there is no enumeration of them anywhere and you must not add one.
- A new setting is not done until it is reachable every way the others are (builder, `hegel.toml`, C ABI, `HEGEL_*` env var, docs) and plumbed through the frontends; follow the `new-setting` skill.
